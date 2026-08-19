// Unit tests for the shared runtime.
//
// These cover the pieces the golden test exercises only end to end: the parser
// on markup shapes that broke it, the selector engine on the constructs the
// committed descriptors use, and the coercions that have to match Rust exactly.

import { describe, expect, test } from "bun:test";
import { parseDocument, textOf, descendants, elementChildren, decodeEntities } from "./html";
import { compileSelector, selectAll, selectFirst, UnsupportedSelectorError } from "./css";
import { normalizeForExtraction, htmlToText } from "./normalize";
import { extractItems, Extractor, formatExtractedJson, getNumber, getText } from "./extract";
import { missingParams, missingParamsError, EX_USAGE, Param } from "./params";
import { execResultFromHtml } from "./exec";

const root = (html: string) => parseDocument(html);

describe("html parser", () => {
  test("void elements do not swallow their siblings", () => {
    // A parser that pushes <br> on the stack nests everything after it, which
    // turns a flat row of cells into a staircase.
    const doc = root("<div><br><span>a</span><span>b</span></div>");
    const spans = selectAll(doc, compileSelector("span"), doc);
    expect(spans.length).toBe(2);
    expect(elementChildren(spans[0]).length).toBe(0);
  });

  test("a > inside a quoted attribute does not end the tag", () => {
    const doc = root(`<a href="x" onclick="if(a>b){f()}">text</a>`);
    const anchor = selectFirst(doc, compileSelector("a"), doc);
    expect(anchor).toBeDefined();
    expect(textOf(anchor!)).toBe("text");
    expect(anchor!.attrs.get("href")).toBe("x");
  });

  test("script contents are not parsed as markup", () => {
    const doc = root("<div><script>var a = 1 < 2;</script><p>after</p></div>");
    const paragraphs = selectAll(doc, compileSelector("p"), doc);
    expect(paragraphs.length).toBe(1);
    expect(textOf(paragraphs[0])).toBe("after");
  });

  test("unclosed td and tr get implied end tags", () => {
    // HN leaves cells unclosed. Without implied ends they nest, and
    // td:last-child then names the innermost cell rather than the last one.
    const doc = root("<table><tr><td>one<td>two<tr><td>three</table>");
    const cells = selectAll(doc, compileSelector("td"), doc);
    expect(cells.length).toBe(3);
    expect(textOf(cells[0])).toBe("one");
    expect(textOf(cells[2])).toBe("three");
  });

  test("single-quoted and unquoted attributes both parse", () => {
    const doc = root("<a href='x' id=y class=z>t</a>");
    const anchor = selectFirst(doc, compileSelector("a"), doc)!;
    expect(anchor.attrs.get("href")).toBe("x");
    expect(anchor.attrs.get("id")).toBe("y");
    expect(anchor.attrs.get("class")).toBe("z");
  });

  test("entities decode in text and attributes", () => {
    expect(decodeEntities("a&amp;b")).toBe("a&b");
    expect(decodeEntities("&lt;tag&gt;")).toBe("<tag>");
    expect(decodeEntities("&#65;&#x42;")).toBe("AB");
    // An unterminated ampersand is content, not a broken entity.
    expect(decodeEntities("Fish & Chips")).toBe("Fish & Chips");
  });

  test("comments carry no selectable structure", () => {
    const doc = root("<div><!-- <p>ghost</p> --><p>real</p></div>");
    expect(selectAll(doc, compileSelector("p"), doc).length).toBe(1);
  });
});

describe("css selectors", () => {
  const doc = root(`
    <table>
      <tr class="athing" id="r1"><td><span>1.</span></td><td><a href="https://a.example/x">Alpha</a></td></tr>
      <tr class="athing" id="r2"><td><span>2.</span></td><td><a href="/relative">Beta</a></td></tr>
    </table>
  `);

  test("tag, class, and id select the expected elements", () => {
    expect(selectAll(doc, compileSelector("tr"), doc).length).toBe(2);
    expect(selectAll(doc, compileSelector(".athing"), doc).length).toBe(2);
    expect(selectAll(doc, compileSelector("#r2"), doc).length).toBe(1);
  });

  test("attribute prefix matching distinguishes absolute from relative hrefs", () => {
    const absolute = selectAll(doc, compileSelector('a[href^="https://"]'), doc);
    expect(absolute.length).toBe(1);
    expect(textOf(absolute[0])).toBe("Alpha");
  });

  test(":has with a child combinator anchors at the subject", () => {
    // This is the construct the committed HN descriptor depends on. Matching it
    // unanchored would select every ancestor table row too.
    const rows = selectAll(doc, compileSelector('tr:has(> td > a[href^="https://"])'), doc);
    expect(rows.length).toBe(1);
    expect(rows[0].attrs.get("id")).toBe("r1");
  });

  test("first-child and last-child address the right cells", () => {
    const firsts = selectAll(doc, compileSelector("td:first-child span"), doc);
    expect(firsts.length).toBe(2);
    expect(textOf(firsts[0])).toBe("1.");

    const lasts = selectAll(doc, compileSelector("td:last-child a"), doc);
    expect(lasts.length).toBe(2);
    expect(textOf(lasts[0])).toBe("Alpha");
  });

  test("first-of-type counts only same-tag siblings", () => {
    const one = root("<p><span>s</span><a>first</a><a>second</a></p>");
    const anchor = selectFirst(one, compileSelector("a:first-of-type"), one)!;
    expect(textOf(anchor)).toBe("first");
  });

  test("child and descendant combinators differ", () => {
    const nested = root("<div><p><b>deep</b></p></div>");
    expect(selectAll(nested, compileSelector("div > b"), nested).length).toBe(0);
    expect(selectAll(nested, compileSelector("div b"), nested).length).toBe(1);
  });

  test("nth-child understands an+b, odd, and even", () => {
    const list = root("<ul><li>1</li><li>2</li><li>3</li><li>4</li></ul>");
    expect(selectAll(list, compileSelector("li:nth-child(odd)"), list).length).toBe(2);
    expect(selectAll(list, compileSelector("li:nth-child(2n)"), list).length).toBe(2);
    expect(textOf(selectFirst(list, compileSelector("li:nth-child(3)"), list)!)).toBe("3");
  });

  test("an unsupported selector raises rather than matching nothing", () => {
    // Silently matching nothing would be indistinguishable from a page that
    // changed, which is the failure this runtime exists to make legible.
    expect(() => compileSelector("p::before")).toThrow(UnsupportedSelectorError);
    expect(() => compileSelector(":hover")).toThrow(UnsupportedSelectorError);
  });
});

describe("normalization", () => {
  test("an anchor with no text is dropped, one with text is kept", () => {
    // This is the exact transform that makes td:last-child resolve to the story
    // link instead of the upvote control.
    const doc = root(
      '<td class="votelinks"><center><a href="vote?id=1"><div class="votearrow"></div></a></center></td>' +
        '<td><a href="https://x.example">Story</a></td>',
    );
    normalizeForExtraction(doc);
    const anchors = selectAll(doc, compileSelector("a"), doc);
    expect(anchors.length).toBe(1);
    expect(anchors[0].attrs.get("href")).toBe("https://x.example");
  });

  test("an anchor wrapping only an image survives", () => {
    const doc = root('<a href="/x"><img src="y.png"></a>');
    normalizeForExtraction(doc);
    expect(selectAll(doc, compileSelector("a"), doc).length).toBe(1);
  });

  test("script and form are removed", () => {
    const doc = root("<div><script>x</script><form><input></form><p>keep</p></div>");
    normalizeForExtraction(doc);
    expect(selectAll(doc, compileSelector("script"), doc).length).toBe(0);
    expect(selectAll(doc, compileSelector("form"), doc).length).toBe(0);
    expect(selectAll(doc, compileSelector("p"), doc).length).toBe(1);
  });

  test("a div is unwrapped, keeping the text it held", () => {
    const doc = root("<td><div>inner</div></td>");
    normalizeForExtraction(doc);
    const cell = selectFirst(doc, compileSelector("td"), doc)!;
    expect(selectAll(doc, compileSelector("div"), doc).length).toBe(0);
    expect(textOf(cell)).toBe("inner");
  });

  test("htmlToText mirrors the Rust collapse", () => {
    expect(htmlToText("<p>one</p><p>two</p>")).toBe("one\ntwo");
    expect(htmlToText("a<br>b")).toBe("a\nb");
    expect(htmlToText("<p>&amp;&nbsp;x</p>")).toBe("& x");
  });
});

/** The committed HN `news` extractor, spelled the way the emitter writes it. */
const HN_EXTRACTOR: Extractor = {
  type: "list",
  itemPattern: {
    strategy: "css",
    cssSelector: 'tr:has(> td > span > a[href^="https://"])',
    axRole: "",
    axNamePattern: "",
  },
  fields: [
    {
      name: "rank",
      fieldType: "text",
      source: { from: "css", selector: "td:first-child span", attribute: "", role: "", namePattern: "", property: "" },
    },
    {
      name: "title",
      fieldType: "text",
      source: { from: "css", selector: "td:last-child a:first-of-type", attribute: "", role: "", namePattern: "", property: "" },
    },
    {
      name: "url",
      fieldType: "url",
      source: { from: "css", selector: "td:last-child a:first-of-type", attribute: "href", role: "", namePattern: "", property: "" },
    },
  ],
  pagination: { strategy: "queryParam", nextCssSelector: "", pageParam: "" },
};

const HN_ROW =
  '<table><tr class="athing"><td align="right" class="title"><span class="rank">1.</span></td>' +
  '<td class="votelinks"><center><a href="vote?id=1"><div class="votearrow"></div></a></center></td>' +
  '<td class="title"><span class="titleline"><a href="https://example.com/story">The Story</a>' +
  '<span class="sitebit"> (<a href="from?site=example.com"><span class="sitestr">example.com</span></a>)</span>' +
  "</span></td></tr></table>";

describe("extractor", () => {
  test("the HN pattern extracts rank, title, and url from raw markup", () => {
    const outcome = extractItems(HN_ROW, HN_EXTRACTOR);
    expect(outcome.items).toBeDefined();
    const items = outcome.items!;
    expect(items.length).toBe(1);
    expect(items[0].index).toBe(1);
    expect(getText(items[0], "rank")).toBe("1.");
    expect(getText(items[0], "title")).toBe("The Story");
    expect(items[0].fields.url).toEqual({ type: "url", value: "https://example.com/story" });
  });

  test("a raw extractor declines, sending the caller to the text path", () => {
    const outcome = extractItems(HN_ROW, {
      type: "raw",
      itemPattern: { strategy: "css", cssSelector: "", axRole: "", axNamePattern: "" },
      fields: [],
      pagination: { strategy: "queryParam", nextCssSelector: "", pageParam: "" },
    });
    expect(outcome.items).toBeUndefined();
    expect(outcome.failure?.reason).toBe("notAList");
  });

  test("an AX-only pattern reports a named reason, not an empty result", () => {
    const outcome = extractItems(HN_ROW, {
      type: "list",
      itemPattern: { strategy: "axTree", cssSelector: "", axRole: "row", axNamePattern: "" },
      fields: [],
      pagination: { strategy: "queryParam", nextCssSelector: "", pageParam: "" },
    });
    expect(outcome.items).toBeUndefined();
    expect(outcome.failure?.reason).toBe("noCssSelector");
  });

  test("a selector that matches nothing is distinguishable from a bad selector", () => {
    const noMatch = extractItems("<p>nothing here</p>", HN_EXTRACTOR);
    expect(noMatch.failure?.reason).toBe("noMatches");

    const bad = extractItems(HN_ROW, {
      ...HN_EXTRACTOR,
      itemPattern: { strategy: "css", cssSelector: "tr::first-line", axRole: "", axNamePattern: "" },
    });
    expect(bad.failure?.reason).toBe("unsupportedSelector");
  });

  test("number coercion keeps digits and falls back to text, as Rust does", () => {
    const numeric: Extractor = {
      type: "list",
      itemPattern: { strategy: "css", cssSelector: "li", axRole: "", axNamePattern: "" },
      fields: [
        {
          name: "points",
          fieldType: "number",
          source: { from: "css", selector: "span", attribute: "", role: "", namePattern: "", property: "" },
        },
      ],
      pagination: { strategy: "queryParam", nextCssSelector: "", pageParam: "" },
    };

    const items = extractItems(
      "<ul><li><span>104 points</span></li><li><span>1.2.3</span></li></ul>",
      numeric,
    ).items!;
    expect(getNumber(items[0], "points")).toBe(104);
    // "1.2.3" survives the digit filter but is not an f64, so Rust stores the
    // original text. Matching that keeps the two paths' JSON identical.
    expect(items[1].fields.points).toEqual({ type: "text", value: "1.2.3" });
  });

  test("a dateTime field serializes as text, matching ExtractedValue", () => {
    const dated: Extractor = {
      type: "list",
      itemPattern: { strategy: "css", cssSelector: "li", axRole: "", axNamePattern: "" },
      fields: [
        {
          name: "when",
          fieldType: "dateTime",
          source: { from: "css", selector: "time", attribute: "", role: "", namePattern: "", property: "" },
        },
      ],
      pagination: { strategy: "queryParam", nextCssSelector: "", pageParam: "" },
    };
    const items = extractItems("<ul><li><time>2026-01-01</time></li></ul>", dated).items!;
    expect(items[0].fields.when).toEqual({ type: "text", value: "2026-01-01" });
  });

  test("an item with every field missing is dropped", () => {
    const items = extractItems(
      "<table><tr><td><span>1.</span></td><td><a href='https://x.example'>t</a></td></tr>" +
        "<tr><td>nothing</td></tr></table>",
      {
        ...HN_EXTRACTOR,
        itemPattern: { strategy: "css", cssSelector: "tr", axRole: "", axNamePattern: "" },
      },
    ).items!;
    // The second row carries none of the declared fields, so it is not an item.
    expect(items.length).toBe(1);
  });

  test("the JSON envelope matches format_extracted_json", () => {
    const items = extractItems(HN_ROW, HN_EXTRACTOR).items!;
    const json = JSON.parse(formatExtractedJson(items, "https://u.example", "Title"));
    expect(Object.keys(json)).toEqual(["url", "title", "itemCount", "items"]);
    expect(json.itemCount).toBe(1);
    // serde emits BTreeMap keys in order, so the TS side sorts to match.
    expect(Object.keys(json.items[0].fields)).toEqual(["rank", "title", "url"]);
  });

  test("an absent field reads as empty rather than throwing", () => {
    // A compiled binary traps on a missing record key, so every accessor has to
    // go through hasField. This is that guarantee, checked.
    const items = extractItems(HN_ROW, HN_EXTRACTOR).items!;
    expect(getText(items[0], "author")).toBe("");
    expect(Number.isNaN(getNumber(items[0], "points"))).toBe(true);
  });
});

describe("parameter validation", () => {
  const required: Param[] = [
    { name: "id", example: "Hunter17", observations: 1, varies: false },
    { name: "sometimes", example: "", observations: 0, varies: false },
  ];

  test("a parameter observed in every request is required", () => {
    const missing = missingParams(required, new Set<string>());
    expect(missing.length).toBe(1);
    expect(missing[0].name).toBe("id");
  });

  test("a parameter never observed is never enforced", () => {
    const missing = missingParams(required, new Set<string>(["id"]));
    expect(missing.length).toBe(0);
  });

  test("the error body carries the evidence, not just the name", () => {
    const body = missingParamsError("user", missingParams(required, new Set<string>()));
    expect(body.error).toBe("missing parameter");
    expect(body.command).toBe("user");
    expect(body.missing[0]).toEqual({
      name: "id",
      example: "Hunter17",
      observedIn: 1,
      callerControlled: false,
    });
    expect(body.hint).toBe("user id=Hunter17");
  });

  test("a parameter with no example still produces a usable hint", () => {
    const body = missingParamsError("op", [
      { name: "q", example: "", observations: 2, varies: true },
    ]);
    expect(body.hint).toBe("op q=<value>");
  });

  test("EX_USAGE is the sysexits code for a malformed invocation", () => {
    expect(EX_USAGE).toBe(64);
  });
});

describe("exec result", () => {
  test("the shape matches ExecResult in execute.rs", () => {
    const result = execResultFromHtml(200, "https://u.example", "T", "<p>one two</p><p>three</p>");
    expect(Object.keys(result)).toEqual(["status", "url", "title", "content", "wordCount"]);
    expect(result.content).toBe("one two\nthree");
    // Counted here, taken from defuddle on the Rust path. The difference is
    // documented in README.md rather than papered over.
    expect(result.wordCount).toBe(3);
  });
});
