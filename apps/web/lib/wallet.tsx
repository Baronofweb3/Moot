"use client";
import { createContext, useCallback, useContext, useEffect, useRef, useState, ReactNode } from "react";
import { setupWalletSelector, WalletSelector } from "@near-wallet-selector/core";
import { setupModal, WalletSelectorModal } from "@near-wallet-selector/modal-ui";
import { setupMyNearWallet } from "@near-wallet-selector/my-near-wallet";
import { setupMeteorWallet } from "@near-wallet-selector/meteor-wallet";
import { NETWORK } from "./config";

export interface Call { receiverId: string; method: string; args: object; deposit?: string; gas: string }
interface Ctx {
  accountId: string | null; ready: boolean;
  signIn: () => void; signOut: () => Promise<void>;
  send: (calls: Call[], callbackUrl?: string) => Promise<void>;
}
const WalletCtx = createContext<Ctx>({ accountId: null, ready: false, signIn() {}, async signOut() {}, async send() {} });
export const useWallet = () => useContext(WalletCtx);

export function WalletProvider({ children }: { children: ReactNode }) {
  const selectorRef = useRef<WalletSelector | null>(null);
  const modalRef = useRef<WalletSelectorModal | null>(null);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let sub: { unsubscribe: () => void } | undefined;
    (async () => {
      // No contractId on purpose: full-access sign-in, because withdrawals and tips attach 1 yoctoNEAR,
      // which a function-call key cannot do.
      const selector = await setupWalletSelector({
        network: NETWORK,
        modules: [setupMyNearWallet(), setupMeteorWallet()],
      });
      selectorRef.current = selector;
      modalRef.current = setupModal(selector, { theme: "dark", description: "Connect a NEAR testnet wallet to trade on MOOT." });
      const update = () => setAccountId(selector.store.getState().accounts.find((a) => a)?.accountId ?? null);
      update();
      sub = selector.store.observable.subscribe(update);
      setReady(true);
    })().catch((e) => console.error("wallet init failed", e));
    return () => sub?.unsubscribe();
  }, []);

  const signIn = useCallback(() => modalRef.current?.show(), []);
  const signOut = useCallback(async () => {
    const sel = selectorRef.current;
    if (!sel) return;
    const w = await sel.wallet();
    await w.signOut();
    setAccountId(null);
  }, []);

  const send = useCallback(async (calls: Call[], callbackUrl?: string) => {
    const sel = selectorRef.current;
    if (!sel) throw new Error("Wallet not ready");
    const wallet = await sel.wallet();
    // Group consecutive calls to the same receiver into one transaction with several actions.
    const txs: { receiverId: string; actions: any[] }[] = [];
    for (const c of calls) {
      const action = { type: "FunctionCall", params: { methodName: c.method, args: c.args, gas: c.gas, deposit: c.deposit ?? "0" } };
      const last = txs[txs.length - 1];
      if (last && last.receiverId === c.receiverId) last.actions.push(action);
      else txs.push({ receiverId: c.receiverId, actions: [action] });
    }
    await (wallet as any).signAndSendTransactions({ transactions: txs, callbackUrl });
  }, []);

  return <WalletCtx.Provider value={{ accountId, ready, signIn, signOut, send }}>{children}</WalletCtx.Provider>;
}
