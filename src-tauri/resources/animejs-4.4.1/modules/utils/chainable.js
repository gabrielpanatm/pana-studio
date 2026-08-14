/**
 * Anime.js - utils - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{noop as l}from"../core/consts.js";import*as g from"./number.js";const n=g,p={},i=(o,r=0)=>(...d)=>r?t=>o(...d,t):t=>o(t,...d),s=o=>(...r)=>{const d=o(...r);return new Proxy(l,{apply:(t,e,[c])=>d(c),get:(t,e)=>{if(p[e])return s((...c)=>{const u=p[e](...c);return m=>u(d(m))})}})},a=(o,r,d=0)=>{const t=(...e)=>(e.length<r.length?s(i(r,d)):r)(...e);return p[o]||(p[o]=t),t},R=a("roundPad",n.roundPad),h=a("padStart",n.padStart),T=a("padEnd",n.padEnd),b=a("wrap",n.wrap),w=a("mapRange",n.mapRange),P=a("degToRad",n.degToRad),_=a("radToDeg",n.radToDeg),x=a("snap",n.snap),y=a("clamp",n.clamp),D=a("round",n.round),E=a("lerp",n.lerp,1),S=a("damp",n.damp,1);export{y as clamp,S as damp,P as degToRad,E as lerp,w as mapRange,T as padEnd,h as padStart,_ as radToDeg,D as round,R as roundPad,x as snap,b as wrap};
