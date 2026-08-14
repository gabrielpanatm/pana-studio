/**
 * Anime.js - svg - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{round as N}from"../core/helpers.js";import{getPath as P}from"./helpers.js";const w=(x,a=.33)=>(t,L,d,i)=>{if(!(t.tagName||"").toLowerCase().match(/^(path|polygon|polyline)$/))throw new Error(`Can't morph a <${t.tagName}> SVG element. Use <path>, <polygon> or <polyline>.`);const o=P(x);if(!o)throw new Error("Can't morph to an invalid target. 'path2' must resolve to an existing <path>, <polygon> or <polyline> SVG element.");if(!(o.tagName||"").toLowerCase().match(/^(path|polygon|polyline)$/))throw new Error(`Can't morph a <${o.tagName}> SVG element. Use <path>, <polygon> or <polyline>.`);const e=t.tagName==="path",g=e?" ":",",s=i?i._value:null;s&&t.setAttribute(e?"d":"points",s);let r="",l="";if(!a)r=t.getAttribute(e?"d":"points"),l=o.getAttribute(e?"d":"points");else{const m=t.getTotalLength(),h=o.getTotalLength(),p=Math.max(Math.ceil(m*a),Math.ceil(h*a));for(let n=0;n<p;n++){const c=n/(p-1),y=t.getPointAtLength(m*c),f=o.getPointAtLength(h*c),u=e?n===0?"M":"L":"";r+=u+N(y.x,3)+g+y.y+" ",l+=u+N(f.x,3)+g+f.y+" "}}return[r,l]};export{w as morphTo};
