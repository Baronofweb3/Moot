export const NETWORK = "testnet" as const;
export const CORE = process.env.NEXT_PUBLIC_MOOT_CORE || "moot-core-01.testnet";
export const WNEAR = process.env.NEXT_PUBLIC_WNEAR || "wrap.testnet";
export const RPC_URLS = [
  process.env.NEXT_PUBLIC_RPC_URL || "https://archival-rpc.testnet.fastnear.com",
  "https://rpc.testnet.fastnear.com",
  "https://rpc.testnet.near.org",
];
export const EXPLORER = "https://testnet.nearblocks.io";
export const GAS = { TRANSFER_CALL: "150000000000000", CREATE: "150000000000000", SIMPLE: "50000000000000", WRAP: "30000000000000" };
export const ONE_YOCTO = "1";
/** Storage registration deposit; the unused part of a registration_only deposit is refunded. */
export const STORAGE_DEPOSIT = "10000000000000000000000"; // 0.01 NEAR
