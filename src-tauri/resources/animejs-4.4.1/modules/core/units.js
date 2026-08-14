/**
 * Anime.js - core - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{valueTypes as b,doc as N}from"./consts.js";import{isUnd as w,PI as C}from"./helpers.js";const o={deg:1,rad:180/C,turn:360},d={},I=(a,t,n,v=!1)=>{const e=t.u,c=t.n;if(t.t===b.UNIT&&e===n)return t;const l=c+e+n,f=d[l];if(!w(f)&&!v)t.n=f;else{let s;if(e in o)s=c*o[e]/o[n];else{const r=a.cloneNode(),i=a.parentNode,h=i&&i!==N?i:N.body;h.appendChild(r);const U=r.style;U.width=100+e;const y=r.offsetWidth||100;U.width=100+n;const W=r.offsetWidth||100,p=y/W;h.removeChild(r),s=p*c}t.n=s,d[l]=s}return t.t,b.UNIT,t.u=n,t};export{I as convertValueUnit};
