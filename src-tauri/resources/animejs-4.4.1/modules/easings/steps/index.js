/**
 * Anime.js - easings - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{ceil as e,floor as n,clamp as l}from"../../core/helpers.js";const m=(o=10,r)=>{const t=r?e:n;return c=>t(l(c,0,1)*o)*(1/o)};export{m as steps};
