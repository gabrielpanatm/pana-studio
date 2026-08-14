/**
 * Anime.js - core - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{rgbExecRgx as E,rgbaExecRgx as T,hslExecRgx as d,hslaExecRgx as a}from"./consts.js";import{isRgb as h,isHex as H,isHsl as v,isUnd as f,round as R}from"./helpers.js";const y=s=>{const o=E.exec(s)||T.exec(s),n=f(o[4])?1:+o[4];return[+o[1],+o[2],+o[3],n]},A=s=>{const o=s.length,n=o===4||o===5;return[+("0x"+s[1]+s[n?1:2]),+("0x"+s[n?2:3]+s[n?2:4]),+("0x"+s[n?3:5]+s[n?3:6]),o===5||o===9?+(+("0x"+s[n?4:7]+s[n?4:8])/255).toFixed(3):1]},i=(s,o,n)=>(n<0&&(n+=1),n>1&&(n-=1),n<1/6?s+(o-s)*6*n:n<1/2?o:n<2/3?s+(o-s)*(2/3-n)*6:s),C=s=>{const o=d.exec(s)||a.exec(s),n=+o[1]/360,t=+o[2]/100,c=+o[3]/100,m=f(o[4])?1:+o[4];let x,e,g;if(t===0)x=e=g=c;else{const r=c<.5?c*(1+t):c+t-c*t,b=2*c-r;x=R(i(b,r,n+1/3)*255,0),e=R(i(b,r,n)*255,0),g=R(i(b,r,n-1/3)*255,0)}return[x,e,g,m]},F=s=>h(s)?y(s):H(s)?A(s):v(s)?C(s):[0,0,0,1];export{F as convertColorStringValuesToRgbaArray};
