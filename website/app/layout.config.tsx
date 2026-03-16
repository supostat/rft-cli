import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import { Terminal } from "lucide-react";

export const baseOptions: BaseLayoutProps = {
  nav: {
    title: (
      <>
        <Terminal className="size-5" />
        <span className="font-semibold">rft</span>
      </>
    ),
  },
  links: [
    {
      text: "Documentation",
      url: "/docs",
      active: "nested-url",
    },
    {
      text: "GitHub",
      url: "https://github.com/ingvar/rft",
      external: true,
    },
  ],
};
