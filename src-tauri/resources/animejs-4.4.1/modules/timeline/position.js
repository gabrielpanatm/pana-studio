/**
 * Anime.js - timeline - ESM
 * @version v4.4.1
 * @license MIT
 * @copyright 2026 - Julian Garnier
 */import{relativeValuesExecRgx as d,minValue as m}from"../core/consts.js";import{isUnd as f,isNum as g,stringStartsWith as b,isNil as h}from"../core/helpers.js";import{getRelativeValue as x}from"../core/values.js";const N=(n,s)=>{if(b(s,"<")){const e=s[1]==="<",t=n._tail,r=t?t._offset+t._delay:0;return e?r:r+t.duration}},R=(n,s)=>{let e=n.iterationDuration;if(e===m&&(e=0),f(s))return e;if(g(+s))return+s;const t=s,r=n?n.labels:null,o=!h(r),a=N(n,t),i=!f(a),c=d.exec(t);if(c){const u=c[0],l=t.split(u),p=o&&l[0]?r[l[0]]:e,O=i?a:o?p:e,v=+l[1];return x(O,v,u[0])}else return i?a:o?f(r[t])?e:r[t]:e};export{R as parseTimelinePosition};
