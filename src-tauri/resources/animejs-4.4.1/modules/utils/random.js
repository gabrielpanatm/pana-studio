/**
 * Anime.js - utils - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */const a=(t=0,r=1,e=0)=>{const o=10**e;return Math.floor((Math.random()*(r-t+1/o)+t)*o)/o};let u=0;const f=(t,r=0,e=1,o=0)=>{let n=t===void 0?u++:t;return(h=r,c=e,d=o)=>{n+=1831565813,n=Math.imul(n^n>>>15,n|1),n^=n+Math.imul(n^n>>>7,n|61);const l=10**d;return Math.floor((((n^n>>>14)>>>0)/4294967296*(c-h+1/l)+h)*l)/l}},M=t=>t[a(0,t.length-1)],g=t=>{let r=t.length,e,o;for(;r;)o=a(0,--r),e=t[r],t[r]=t[o],t[o]=e;return t};export{f as createSeededRandom,a as random,M as randomPick,g as shuffle};
