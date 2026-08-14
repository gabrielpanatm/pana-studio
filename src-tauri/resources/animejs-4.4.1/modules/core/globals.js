/**
 * Anime.js - core - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{isBrowser as s,win as o,noop as e,compositionTypes as i,K as t,maxFps as r,doc as p}from"./consts.js";const n={id:null,keyframes:null,playbackEase:null,playbackRate:1,frameRate:r,loop:0,reversed:!1,alternate:!1,autoplay:!0,persist:!1,duration:t,delay:0,loopDelay:0,ease:"out(2)",composition:i.replace,modifier:a=>a,onBegin:e,onBeforeUpdate:e,onUpdate:e,onLoop:e,onPause:e,onComplete:e,onRender:e},c={current:null,root:p},u={defaults:n,precision:4,timeScale:1,tickThreshold:200,editor:null},l={version:"4.4.1",engine:null};s&&(o.AnimeJS||(o.AnimeJS=[]),o.AnimeJS.push(l));export{n as defaults,l as globalVersions,u as globals,c as scope};
