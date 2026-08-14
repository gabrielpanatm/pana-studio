/**
 * Anime.js - svg - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{proxyTargetSymbol as C,K as L}from"../core/consts.js";import{isFnc as x,sqrt as k}from"../core/helpers.js";import{parseTargets as g}from"../core/targets.js";const F=e=>{let c=1;if(e&&e.getCTM){const t=e.getCTM();if(t){const s=k(t.a*t.a+t.b*t.b),r=k(t.c*t.c+t.d*t.d);c=(s+r)/2}}return c},S=(e,c,t)=>{const s=L,r=getComputedStyle(e),p=r.strokeLinecap,w=r.vectorEffect==="non-scaling-stroke"?e:null;let d=p;const m=new Proxy(e,{get(o,f){const a=o[f];return f===C?o:f==="setAttribute"?(...n)=>{if(n[0]==="draw"){const y=n[1].split(" "),i=+y[0],l=+y[1],u=F(w),h=i*-s*u,v=l*s*u+h,A=s*u+(i===0&&l===1||i===1&&l===0?0:10*u)-v;if(p!=="butt"){const b=i===l?"butt":p;d!==b&&(o.style.strokeLinecap=`${b}`,d=b)}o.setAttribute("stroke-dashoffset",`${h}`),o.setAttribute("stroke-dasharray",`${v} ${A}`)}return Reflect.apply(a,o,n)}:x(a)?(...n)=>Reflect.apply(a,o,n):a}});return e.getAttribute("pathLength")!==`${s}`&&(e.setAttribute("pathLength",`${s}`),m.setAttribute("draw",`${c} ${t}`)),m},T=(e,c=0,t=0)=>g(e).map(r=>S(r,c,t));export{T as createDrawable};
