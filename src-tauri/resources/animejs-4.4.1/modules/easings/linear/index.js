/**
 * Anime.js - easings - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{isStr as g,parseNumber as c,isUnd as v}from"../../core/helpers.js";import{none as P}from"../none.js";const x=(...i)=>{const u=i.length;if(!u)return P;const l=u-1,a=i[0],m=i[l],o=[0],n=[c(a)];for(let s=1;s<l;s++){const r=i[s],t=g(r)?r.trim().split(" "):[r],p=t[0],e=t[1];o.push(v(e)?s/l:c(e)/100),n.push(c(p))}return n.push(c(m)),o.push(1),function(r){for(let t=1,p=o.length;t<p;t++){const e=o[t];if(r<=e){const f=o[t-1],h=n[t-1];return h+(n[t]-h)*(r-f)/(e-f)}}return n[n.length-1]}};export{x as linear};
