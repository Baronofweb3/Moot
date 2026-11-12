import "./globals.css";
import type { Metadata } from "next";
import { WalletProvider } from "@/lib/wallet";
export const metadata: Metadata = { title: "MOOT Control Room", description: "Protocol operations for MOOT (testnet).", robots: { index: false, follow: false } };
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (<html lang="en"><body><WalletProvider>{children}</WalletProvider></body></html>);
}
