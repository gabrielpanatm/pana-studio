/**
 * Anime.js - easings - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{abs as b}from"../../core/helpers.js";import{none as l}from"../none.js";const s=(e,r,i)=>(((1-3*i+3*r)*e+(3*i-6*r))*e+3*r)*e,d=(e,r,i)=>{let c=0,o=1,t,n,u=0;do n=c+(o-c)/2,t=s(n,r,i)-e,t>0?o=n:c=n;while(b(t)>1e-7&&++u<100);return n},f=(e=.5,r=0,i=.5,c=1)=>e===r&&i===c?l:o=>o===0||o===1?o:s(d(o,e,i),r,c);export{f as cubicBezier};
