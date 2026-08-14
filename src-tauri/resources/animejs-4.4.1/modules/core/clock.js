/**
 * Anime.js - core - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{K as h,maxFps as o,minValue as r,tickModes as l}from"./consts.js";import{defaults as m}from"./globals.js";class u{constructor(t=0){this.deltaTime=0,this._currentTime=t,this._lastTickTime=t,this._startTime=t,this._lastTime=t,this._scheduledTime=0,this._frameDuration=h/o,this._fps=o,this._speed=1,this._hasChildren=!1,this._head=null,this._tail=null}get fps(){return this._fps}set fps(t){const s=this._frameDuration,i=+t,e=i<r?r:i,a=h/e;e>m.frameRate&&(m.frameRate=e),this._fps=e,this._frameDuration=a,this._scheduledTime+=a-s}get speed(){return this._speed}set speed(t){const s=+t;this._speed=s<r?r:s}requestTick(t){const s=this._scheduledTime;if(this._lastTickTime=t,t<s)return l.NONE;const i=this._frameDuration,e=t-s;return this._scheduledTime+=e<i?i:e,l.AUTO}computeDeltaTime(t){const s=t-this._lastTime;return this.deltaTime=s,this._lastTime=t,s}}export{u as Clock};
