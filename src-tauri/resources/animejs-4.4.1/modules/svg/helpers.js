/**
 * Anime.js - svg - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{isSvg as t}from"../core/helpers.js";import{parseTargets as s}from"../core/targets.js";const o=e=>{const r=s(e)[0];return!r||!t(r)?console.warn(`${e} is not a valid SVGGeometryElement`):r};export{o as getPath};
