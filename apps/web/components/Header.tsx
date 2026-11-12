"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useWallet } from "@/lib/wallet";

export default function Header() {
  const path = usePathname();
  const { accountId, signIn, signOut, ready } = useWallet();
  const link = (href: string, label: string) => (
    <Link href={href} className={path === href || (href !== "/" && path.startsWith(href)) ? "on" : ""}>{label}</Link>
  );
  return (
    <header className="top">
      <div className="wrap">
        <Link href="/" className="brand"><img src="/logo.png" alt="MOOT" /><span>MOOT</span></Link>
        <nav>{link("/", "Discover")}{link("/launch", "Launch")}{link("/dashboard", "Dashboard")}{link("/how", "How it works")}</nav>
        {accountId ? (
          <button className="btn ghost" onClick={() => signOut()} title="Disconnect">{accountId.length > 22 ? accountId.slice(0, 10) + "…" + accountId.slice(-10) : accountId}</button>
        ) : (
          <button className="btn" disabled={!ready} onClick={signIn}>Connect wallet</button>
        )}
      </div>
    </header>
  );
}
