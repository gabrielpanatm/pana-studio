/**
 * Anime.js - waapi - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{removeChild as h,addChild as r}from"../core/helpers.js";const s={_head:null,_tail:null},u=(t,a,l)=>{let n=s._head,i;for(;n;){const o=n._next,m=n.$el===t,c=!a||n.property===a,d=!l||n.parent===l;if(m&&c&&d){i=n.animation;try{i.commitStyles()}catch{}i.cancel(),h(s,n);const e=n.parent;e&&(e._completed++,e.animations.length===e._completed&&(e.completed=!0,e.paused=!0,e.muteCallbacks||(e.onComplete(e),e._resolve(e))))}n=o}return i},A=(t,a,l,n,i)=>{const o=a.animate(n,i),m=i.delay+ +i.duration*i.iterations;o.playbackRate=t._speed,t.paused&&o.pause(),t.duration<m&&(t.duration=m,t.controlAnimation=o),t.animations.push(o),u(a,l),r(s,{parent:t,animation:o,$el:a,property:l,_next:null,_prev:null});const c=()=>u(a,l,t);return o.oncancel=c,o.onremove=c,t.persist||(o.onfinish=c),o};export{A as addWAAPIAnimation,u as removeWAAPIAnimation};
