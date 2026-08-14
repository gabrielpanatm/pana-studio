/**
 * Anime.js - animation - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{noop as d,minValue as c,valueTypes as s,tickModes as v}from"../core/consts.js";import{cloneArray as b}from"../core/helpers.js";import{render as h}from"../core/render.js";const i={animation:null,update:d},y=_=>{let o=i.animation;return o||(o={duration:c,computeDeltaTime:d,_offset:0,_delay:0,_head:null,_tail:null},i.animation=o,i.update=()=>{_.forEach(l=>{for(let f in l){const r=l[f],e=r._head;if(e){const m=e._valueType,a=m===s.COMPLEX||m===s.COLOR?b(e._fromNumbers):null;let u=e._fromNumber,t=r._tail;for(;t&&t!==e;){if(a)for(let n=0,p=t._numbers.length;n<p;n++)a[n]+=t._numbers[n];else u+=t._number;t=t._prevAdd}e._toNumber=u,e._toNumbers=a}}}),h(o,1,1,0,v.FORCE)}),o};export{y as addAdditiveAnimation,i as additive};
