import './global.css';

import { RootProvider } from 'fumadocs-ui/provider/next';
import type { Metadata } from 'next';
import type { ReactNode } from 'react';

export const metadata: Metadata = {
  icons: {
    icon: '/greybound-robine-mark.svg',
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex min-h-screen flex-col">
        <RootProvider>{children}</RootProvider>
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
                  applicationId: 'greybound-docs',
                  proxy: 'https://robinedog.bailleul.dev/rum/proxy',
                  service: 'greybound-docs',
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
