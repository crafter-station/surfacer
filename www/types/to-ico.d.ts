declare module "to-ico" {
  /** Packs PNG buffers into a single ICO with one entry per input. */
  export default function toIco(input: Buffer[]): Promise<Buffer>;
}
