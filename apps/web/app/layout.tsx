import "./globals.css";
import type { Metadata } from "next";
import { WalletProvider } from "@/lib/wallet";
import Header from "@/components/Header";

export const metadata: Metadata = {
  title: "MOOT — Trade together. Earn together.",
  description: "A NEAR-native memecoin launchpad where every trade pays creators and traders. Testnet prototype.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <div className="splash"><div style={{ textAlign: "center" }}><img src="/logo.png" alt="" /><p>Trade together.<br />Earn together.</p></div></div>
        <WalletProvider>
          <Header />
          <main className="wrap">{children}</main>
          <footer className="wrap">MOOT · NEAR testnet prototype · not financial advice · no real funds</footer>
        </WalletProvider>
      </body>
    </html>
  );
}
