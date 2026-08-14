/**
 * Anime.js - utils - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{lerp as o}from"../core/helpers.js";import{clamp as E,round as I,snap as R}from"../core/helpers.js";const n=(p,t)=>(+p).toFixed(t),e=(p,t,a)=>`${p}`.padStart(t,a),s=(p,t,a)=>`${p}`.padEnd(t,a),c=(p,t,a)=>((p-t)%(a-t)+(a-t))%(a-t)+t,g=(p,t,a,d,r)=>d+(p-t)/(a-t)*(r-d),h=p=>p*Math.PI/180,M=p=>p*180/Math.PI,P=(p,t,a,d)=>d?d===1?t:o(p,t,1-Math.exp(-d*a*.1)):p;export{E as clamp,P as damp,h as degToRad,o as lerp,g as mapRange,s as padEnd,e as padStart,M as radToDeg,I as round,n as roundPad,R as snap,c as wrap};
