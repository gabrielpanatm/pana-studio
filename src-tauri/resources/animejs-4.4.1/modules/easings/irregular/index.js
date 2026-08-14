/**
 * Anime.js - easings - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{clamp as m}from"../../core/helpers.js";import{linear as p}from"../linear/index.js";const u=(s=10,a=1)=>{const t=[0],r=s-1;for(let o=1;o<r;o++){const c=t[o-1],n=o/r,e=(o+1)/r,i=n+(e-n)*Math.random(),l=n*(1-a)+i*a;t.push(m(l,c,1))}return t.push(1),p(...t)};export{u as irregular};
