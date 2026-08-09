import type { NextConfig } from "next";

const INSTALLER =
  "https://raw.githubusercontent.com/crafter-station/surfacer/main/install.sh";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      // The site advertises `curl surfacer.dev/install.sh`, which is shorter
      // than the raw GitHub URL and does not tie the install path to a host
      // outside our control. The script itself stays in the repo.
      { source: "/install.sh", destination: INSTALLER },
    ];
  },
};

export default nextConfig;
