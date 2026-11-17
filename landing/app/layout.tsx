import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: "Greybound | Open-source greybox audio experiment",
  description:
    "Greybound is an open-source experiment for greybox guitar tone: circuit ideas, measured behavior, Rust DSP, and transparent model notes.",
  icons: {
    icon: "/greybound-robine-mark.svg",
  },
};

export const viewport: Viewport = {
  themeColor: "#121417",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        {children}
        {/* Real user monitoring, with Datadog's browser SDK, reported to robine-dog. */}
        <script
          dangerouslySetInnerHTML={{
            __html: `
              (function(h,o,u,n,d) {
                h=h[d]=h[d]||{q:[],onReady:function(c){h.q.push(c)}}
                d=o.createElement(u);d.async=1;d.src=n
                n=o.getElementsByTagName(u)[0];n.parentNode.insertBefore(d,n)
              })(window,document,'script','https://www.datadoghq-browser-agent.com/us1/v6/datadog-rum.js','DD_RUM')
              window.DD_RUM.onReady(function() {
                window.DD_RUM.init({
                  clientToken: 'pub5faff949e57e72975180ee3637d73642',
                  applicationId: 'greybound-landing',
                  proxy: 'https://robinedog.bailleul.dev/rum/proxy',
                  service: 'greybound-landing',
                  env: 'prod',
                  sessionSampleRate: 100,
                  sessionReplaySampleRate: 0,
                  trackUserInteractions: true,
                  trackResources: true,
                  trackLongTasks: true,
                });
              })
            `,
          }}
        />
      </body>
    </html>
  );
}
