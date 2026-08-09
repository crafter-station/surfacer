import type { LanguageRegistration } from "shiki";

/**
 * Shiki ships no grammar for CLI help output, so the `help` pane rendered as
 * flat gray next to two highlighted panes.
 *
 * Help text is structured, not prose: section headers, command names, flags,
 * and parameter examples each mean something different. This grammar marks
 * those four so the same theme colors them.
 */
export const helpGrammar: LanguageRegistration = {
  name: "clihelp",
  scopeName: "source.clihelp",
  patterns: [
    // Section headers sit at column zero in caps: USAGE, COMMANDS, FLAGS.
    { match: "^[A-Z][A-Z ]+$", name: "entity.name.type.clihelp" },
    // Long and short flags anywhere in the line.
    {
      match: "(?<![\\w-])--?[a-zA-Z][\\w-]*",
      name: "variable.parameter.clihelp",
    },
    // A `key=value` pair is a parameter with its observed example.
    {
      match: "\\b([a-zA-Z][\\w-]*)(=)(\\S*)",
      captures: {
        "1": { name: "variable.other.clihelp" },
        "2": { name: "punctuation.separator.clihelp" },
        "3": { name: "string.unquoted.clihelp" },
      },
    },
    // Placeholders the reader is meant to substitute.
    { match: "<[a-zA-Z][\\w. ]*>", name: "string.other.placeholder.clihelp" },
    { match: "\\[[^\\]]+\\]", name: "comment.clihelp" },
    // The first token on an indented line is a command name.
    {
      match: "^  ([a-z][\\w-]*(?: [a-z][\\w-]*)*)(?=\\s\\s|$)",
      captures: { "1": { name: "entity.name.function.clihelp" } },
    },
  ],
  repository: {},
};
