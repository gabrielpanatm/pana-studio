/**
 * Anime.js - core - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{scope as T}from"./globals.js";import{isRegisteredTargetSymbol as y,isDomSymbol as b,isSvgSymbol as L,transformsSymbol as D,isBrowser as j}from"./consts.js";import{isSvg as k,isNil as m,isArr as d,isStr as A}from"./helpers.js";function p(e){const r=A(e)?T.root.querySelectorAll(e):e;if(r instanceof NodeList||r instanceof HTMLCollection)return r}function S(e){if(m(e))return[];if(!j)return d(e)&&e.flat(1/0)||[e];if(d(e)){const n=e.flat(1/0),t=[];for(let i=0,c=n.length;i<c;i++){const s=n[i];if(!m(s)){const u=p(s);if(u)for(let o=0,f=u.length;o<f;o++){const l=u[o];if(!m(l)){let g=!1;for(let a=0,h=t.length;a<h;a++)if(t[a]===l){g=!0;break}g||t.push(l)}}else{let o=!1;for(let f=0,l=t.length;f<l;f++)if(t[f]===s){o=!0;break}o||t.push(s)}}}return t}const r=p(e);return r?Array.from(r):[e]}function I(e){const r=S(e),n=r.length;if(n)for(let t=0;t<n;t++){const i=r[t];if(!i[y]){i[y]=!0;const c=k(i);(i.nodeType||c)&&(i[b]=!0,i[L]=c,i[D]={})}}return r}export{p as getNodeList,S as parseTargets,I as registerTargets};
