/**
 * Anime.js - svg - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{isSvgSymbol as h}from"../core/consts.js";import{atan2 as l,PI as p}from"../core/helpers.js";import{getPath as f}from"./helpers.js";const g=(o,t,r,a,s)=>{const c=r+a,n=s?Math.max(0,Math.min(c,t)):(c%t+t)%t;return o.getPointAtLength(n)},x=(o,t,r=0)=>a=>{const s=+o.getTotalLength(),c=a[h],n=o.getCTM(),i=r===0;return{from:0,to:s,modifier:u=>{const y=r*s,m=u+y;if(t==="a"){const e=g(o,s,m,-1,i),P=g(o,s,m,1,i);return l(P.y-e.y,P.x-e.x)*180/p}else{const e=g(o,s,m,0,i);return t==="x"?c||!n?e.x:e.x*n.a+e.y*n.c+n.e:c||!n?e.y:e.x*n.b+e.y*n.d+n.f}}}},M=(o,t=0)=>{const r=f(o);if(r)return{translateX:x(r,"x",t),translateY:x(r,"y",t),rotate:x(r,"a",t)}};export{M as createMotionPath};
