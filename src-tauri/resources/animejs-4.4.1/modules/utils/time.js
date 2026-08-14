/**
 * Anime.js - utils - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{noop as m}from"../core/consts.js";import{globals as l}from"../core/globals.js";import{isFnc as u,isUnd as p}from"../core/helpers.js";import{Timer as f}from"../timer/timer.js";const d=(o=m)=>new f({duration:1*l.timeScale,onComplete:o},null,0).resume(),T=o=>{let r;return(...c)=>{let i,t,n,s,a;r&&(i=r.currentIteration,t=r.iterationProgress,n=r.reversed,s=r._alternate,a=r._startTime,r.revert());const e=o(...c);return e&&!u(e)&&e.revert&&(r=e),p(t)||(r.currentIteration=i,r.iterationProgress=(s&&i%2?!n:n)?1-t:t,r._startTime=a),e||m}};export{T as keepTime,d as sync};
