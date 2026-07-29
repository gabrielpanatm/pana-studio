use super::{MotionDocument, PageJsConfig};

pub fn generate_motion_js(config: &PageJsConfig) -> String {
    let Some(motion) = config.motion.as_ref().filter(|motion| !motion.is_empty()) else {
        return String::new();
    };
    if let Err(error) = motion.validate() {
        return format!(
            "  console.error('[Pană Motion v2] Configurația nu poate fi executată', {});",
            serde_json::to_string(&error)
                .unwrap_or_else(|_| "\"Configurație invalidă\"".to_string())
        );
    }
    generate_motion_runtime(motion)
}

fn generate_motion_runtime(motion: &MotionDocument) -> String {
    let payload = serde_json::to_string(motion).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"  /* PANA MOTION V2 · Anime.js 4.4.1 */
  (function(){{
    'use strict';
    var documentConfig={payload};
    var anime=window.anime||{{}};
    var createTimeline=anime.createTimeline||function(){{return null;}};
    var createScope=anime.createScope||function(){{return null;}};
    var createDraggable=anime.createDraggable||function(){{return null;}};
    var createLayout=anime.createLayout||function(){{return null;}};
    var onScroll=anime.onScroll||function(){{return null;}};
    var stagger=anime.stagger||function(value){{return value;}};
    var splitText=anime.splitText||(anime.text&&anime.text.splitText);
    var svg=anime.svg||{{}};
    var PROGRESS_DURATION=1000;
    var previewOnly=false;
    try{{previewOnly=new URLSearchParams(window.location.search).get('__pana_motion_mode')==='preview';}}catch(error){{}}
    var previousRegistry=window.__panaMotionV2;
    if(previousRegistry&&typeof previousRegistry.destroy==='function'){{
      try{{previousRegistry.destroy();}}catch(error){{}}
    }}
    var registry=window.__panaMotionV2={{}};
    registry.schemaVersion=2;
    registry.animeVersion='4.4.1';
    registry.document=documentConfig;
    registry.instances={{}};
    registry.scopes={{}};
    registry.behaviors={{}};
    registry.splitters=[];
    registry.errors=[];
    registry.cleanups=[];
    registry.effectCleanups=[];
    var instanceIds=new WeakMap();
    var nextInstanceId=1;

    function report(owner,error){{
      var diagnostic={{id:owner&&owner.id||'',message:error&&error.message?error.message:String(error)}};
      registry.errors.push(diagnostic);
      if(window.console&&console.warn)console.warn('[Pană Motion v2]',diagnostic.id,error);
    }}
    function list(value){{
      if(!value)return[];
      if(Array.isArray(value))return value;
      if(typeof value.length==='number'&&typeof value!=='string')return Array.prototype.slice.call(value);
      return[value];
    }}
    function cssEscape(value){{
      if(window.CSS&&typeof window.CSS.escape==='function')return window.CSS.escape(String(value));
      return String(value).replace(/["\\]/g,'\\$&');
    }}
    function query(selector,root){{
      if(!selector)return[];
      try{{return list((root||document).querySelectorAll(selector));}}catch(error){{report({{id:'target'}},error);return[];}}
    }}
    function first(value){{return list(value)[0]||null;}}
    function targetSelector(ref){{
      if(!ref)return'';
      if(ref.kind==='element'&&ref.dataAnim)return'[data-anim="'+cssEscape(ref.dataAnim)+'"]';
      return ref.selector||'';
    }}
    function applyScope(nodes,scope){{
      if(scope==='first')return nodes.length?[nodes[0]]:[];
      return nodes;
    }}
    function relativeTargets(ref,trigger){{
      if(!trigger)return[];
      var selector=ref.selector||'*';
      var nodes=[];
      if(ref.relation==='children')nodes=list(trigger.children).filter(function(node){{return node.matches&&node.matches(selector);}});
      else if(ref.relation==='descendants')nodes=query(selector,trigger);
      else if(ref.relation==='parent')nodes=trigger.parentElement&&trigger.parentElement.matches(selector)?[trigger.parentElement]:[];
      else if(ref.relation==='ancestors'){{var node=trigger.parentElement;while(node){{if(node.matches&&node.matches(selector))nodes.push(node);node=node.parentElement;}}}}
      else if(ref.relation==='siblings')nodes=trigger.parentElement?list(trigger.parentElement.children).filter(function(node){{return node!==trigger&&node.matches&&node.matches(selector);}}):[];
      else if(ref.relation==='nextSibling')nodes=trigger.nextElementSibling&&trigger.nextElementSibling.matches(selector)?[trigger.nextElementSibling]:[];
      else if(ref.relation==='previousSibling')nodes=trigger.previousElementSibling&&trigger.previousElementSibling.matches(selector)?[trigger.previousElementSibling]:[];
      else nodes=trigger.matches&&trigger.matches(selector)?[trigger]:[];
      return applyScope(nodes,ref.scope);
    }}
    function resolveTarget(ref,trigger){{
      if(!ref)return[];
      if(ref.kind==='trigger')return trigger?[trigger]:[];
      if(ref.kind==='viewport')return[window];
      if(ref.kind==='document')return[document];
      if(ref.kind==='relative')return relativeTargets(ref,trigger);
      return applyScope(query(targetSelector(ref),document),ref.scope);
    }}
    function readValue(value){{
      if(!value)return'';
      var raw=value.value==null?'':String(value.value);
      var unit=value.unit||'';
      if(value.kind==='cssVariable')return raw.indexOf('var(')===0?raw:'var('+raw+')';
      if(value.kind==='number'&&!unit&&raw.trim()!==''&&isFinite(Number(raw)))return Number(raw);
      return raw+unit;
    }}
    function propertyValue(property,mode){{
      var to=readValue(property.to);
      var from=property.from?readValue(property.from):undefined;
      if(mode==='fromTo'&&from!==undefined)return[from,to];
      if(mode==='from'&&from!==undefined)return{{from:from}};
      return to;
    }}
    function actionProperties(action){{
      var params={{}};
      (action.properties||[]).forEach(function(property){{
        if(property&&property.name)params[property.name]=propertyValue(property,action.mode);
      }});
      return params;
    }}
    function keyframes(action){{
      if(!action.keyframes||!action.keyframes.length)return null;
      var frames={{}};
      action.keyframes.forEach(function(frame){{
        var values={{}};
        (frame.properties||[]).forEach(function(property){{
          if(property&&property.name)values[property.name]=propertyValue(property,'to');
        }});
        if(frame.ease)values.ease=frame.ease;
        frames[String(Math.max(0,Math.min(100,frame.offset)))+'%']=values;
      }});
      return frames;
    }}
    function applyRepeat(params,repeat){{
      if(!repeat)return params;
      if(repeat.infinite)params.loop=true;
      else if(Number(repeat.count)>0)params.loop=Number(repeat.count);
      if(repeat.alternate)params.alternate=true;
      if(Number(repeat.delayMs)>0)params.loopDelay=Number(repeat.delayMs);
      return params;
    }}
    function timelinePosition(interaction,value){{
      return interaction.domain==='progress'?Number(value||0)/100*PROGRESS_DURATION:Number(value||0);
    }}
    function timelineDuration(interaction,value){{
      var duration=interaction.domain==='progress'?Number(value||0)/100*PROGRESS_DURATION:Number(value||0);
      return interaction.domain!=='progress'&&reducedMode(interaction)==='reduce'?duration*0.2:duration;
    }}
    function naturalDuration(interaction){{
      var max=0;
      (interaction.actions||[]).forEach(function(action){{
        var duration=timelineDuration(interaction,action.duration);
        if(action.type==='animate'&&action.repeat&&!action.repeat.infinite){{
          var repeats=Math.max(0,Number(action.repeat.count)||0);
          duration=duration*(repeats+1)+timelineDuration(interaction,action.repeat.delayMs)*repeats;
        }}
        max=Math.max(max,timelinePosition(interaction,action.start)+duration);
      }});
      return interaction.domain==='progress'?PROGRESS_DURATION:Math.max(1,max);
    }}
    function specializationTarget(interaction,action,nodes){{
      var spec=action.specialization;
      if(!spec)return nodes;
      if(spec.type==='splitText'&&splitText){{
        var fragments=[];
        nodes.forEach(function(node){{
          var split=splitText(node,{{lines:spec.mode==='lines',words:spec.mode==='words',chars:spec.mode==='chars',accessible:true}});
          if(split){{
            registry.splitters.push({{interactionId:interaction.id,instance:split}});
            fragments=fragments.concat(split[spec.mode]||[]);
          }}
        }});
        return fragments.length?fragments:nodes;
      }}
      if(spec.type==='svgPath'&&svg.createMotionPath){{
        var path=first(query(spec.path));
        var motionPath=svg.createMotionPath(path);
        if(motionPath&&spec.autoRotate===false)delete motionPath.rotate;
        return{{nodes:nodes,motionPath:motionPath}};
      }}
      if(spec.type==='svgMorph'&&svg.morphTo){{
        return{{nodes:nodes,morph:svg.morphTo(spec.source,Number(spec.precision)||0.33)}};
      }}
      if(spec.type==='svgDraw'&&svg.createDrawable)return svg.createDrawable(nodes);
      return nodes;
    }}
    function animationParameters(interaction,action,nodes){{
      var params=actionProperties(action);
      params.duration=timelineDuration(interaction,action.duration);
      if(action.ease)params.ease=action.ease;
      var frames=keyframes(action);
      if(frames)params.keyframes=frames;
      if(action.stagger){{
        var options={{}};
        if(action.stagger.from)options.from=action.stagger.from;
        if(action.stagger.reversed)options.reversed=true;
        if(action.stagger.ease)options.ease=action.stagger.ease;
        if(action.stagger.mode==='total')options.total=timelineDuration(interaction,action.stagger.amount);
        params.delay=stagger(
          action.stagger.mode==='total'?0:timelineDuration(interaction,action.stagger.amount),
          options
        );
      }}
      applyRepeat(params,action.repeat);
      var specialized=specializationTarget(interaction,action,nodes);
      if(specialized&&specialized.motionPath)Object.keys(specialized.motionPath).forEach(function(key){{params[key]=specialized.motionPath[key];}});
      if(specialized&&specialized.morph)params.d=specialized.morph;
      return{{targets:specialized&&specialized.nodes||specialized,params:params}};
    }}
    function setAction(timeline,interaction,action,trigger,allowSideEffects,lifecycleId){{
      var targets=resolveTarget(action.target,trigger);
      var properties={{}};
      var sideEffects=[];
      (action.values||[]).forEach(function(value){{
        if(value.type==='property')properties[value.name]=readValue(value.value);
        else sideEffects.push(value);
      }});
      var position=timelinePosition(interaction,action.start);
      if(Object.keys(properties).length&&timeline.set)timeline.set(targets,properties,position);
      if(allowSideEffects&&sideEffects.length&&timeline.call){{
        var restorers=[];
        var active=false;
        var restore=function(){{
          restorers.splice(0).reverse().forEach(function(revert){{try{{revert();}}catch(error){{}}}});
          active=false;
        }};
        trackEffectCleanup(lifecycleId,restore);
        timeline.call(function(){{
          if(timeline.backwards){{restore();return;}}
          if(!active){{
            active=true;
            targets.forEach(function(node){{
              sideEffects.forEach(function(value){{
                if(value.type==='attribute'){{
                  var hadAttribute=node.hasAttribute(value.name);
                  var previousAttribute=node.getAttribute(value.name);
                  restorers.push(function(){{
                    if(hadAttribute)node.setAttribute(value.name,previousAttribute);
                    else node.removeAttribute(value.name);
                  }});
                }}else{{
                  var hadClass=node.classList.contains(value.name);
                  restorers.push(function(){{
                    if(hadClass)node.classList.add(value.name);
                    else node.classList.remove(value.name);
                  }});
                }}
              }});
            }});
          }}
          targets.forEach(function(node){{
            sideEffects.forEach(function(value){{
              if(value.type==='attribute')node.setAttribute(value.name,value.value);
              else if(value.type==='addClass')node.classList.add(value.name);
              else if(value.type==='removeClass')node.classList.remove(value.name);
              else if(value.type==='toggleClass')node.classList.toggle(value.name);
            }});
          }});
        }},position);
      }}
    }}
    function mediaAction(timeline,interaction,action,trigger,lifecycleId){{
      if(!timeline.call)return;
      var restorers=[];
      var active=false;
      var restore=function(){{
        restorers.splice(0).reverse().forEach(function(revert){{try{{revert();}}catch(error){{}}}});
        active=false;
      }};
      trackEffectCleanup(lifecycleId,restore);
      timeline.call(function(){{
        if(timeline.backwards){{restore();return;}}
        resolveTarget(action.target,trigger).forEach(function(node){{
          if(!active){{
            var previousPaused=node.paused;
            var previousTime=Number(node.currentTime)||0;
            restorers.push(function(){{
              if(node.pause)node.pause();
              try{{node.currentTime=previousTime;}}catch(error){{}}
              if(previousPaused===false&&node.play){{
                var resumed=node.play();
                if(resumed&&resumed.catch)resumed.catch(function(){{}});
              }}
            }});
          }}
          if(action.command==='play'&&node.play){{var result=node.play();if(result&&result.catch)result.catch(function(){{}});}}
          else if(action.command==='pause'&&node.pause)node.pause();
          else if(action.command==='reset'){{if(node.pause)node.pause();try{{node.currentTime=0;}}catch(error){{}}}}
          else if(action.command==='toggle'){{if(node.paused&&node.play)node.play();else if(node.pause)node.pause();}}
        }});
        active=true;
      }},timelinePosition(interaction,action.start));
    }}
    function compileCall(code,owner){{
      try{{return new Function('timeline','anime','registry','trigger','\"use strict\";\n'+String(code||''));}}
      catch(error){{report(owner,error);return function(){{}};}}
    }}
    function emitPreviewState(interaction,timeline,phase){{
      if(!previewOnly||!interaction||!timeline||!window.parent)return;
      var rawDuration=Number(timeline.iterationDuration||timeline.duration||naturalDuration(interaction));
      var rawValue=Number(timeline.iterationCurrentTime);
      if(!isFinite(rawValue))rawValue=Number(timeline.currentTime)||0;
      var progress=rawDuration>0?Math.max(0,Math.min(1,rawValue/rawDuration)):0;
      window.parent.postMessage({{
        source:'pana-studio-motion-runtime',
        type:'state',
        phase:phase||'update',
        interactionId:interaction.id,
        value:interaction.domain==='progress'?progress*100:rawValue,
        duration:interaction.domain==='progress'?100:rawDuration,
        progress:progress,
        paused:timeline.paused!==false,
        reversed:timeline.reversed===true
      }},'*');
    }}
    function playbackParameters(interaction,autoplay,mode){{
      var playback=interaction.playback||{{}};
      var params={{autoplay:autoplay===undefined?false:autoplay}};
      if(Number(playback.delayMs)>0)params.delay=Number(playback.delayMs);
      if(playback.infinite)params.loop=true;
      else if(Number(playback.repeat)>0)params.loop=Number(playback.repeat);
      if(Number(playback.loopDelayMs)>0)params.loopDelay=Number(playback.loopDelayMs);
      if(playback.alternate)params.alternate=true;
      if(playback.reversed)params.reversed=true;
      if(Number(playback.playbackRate)>0&&Number(playback.playbackRate)!==1)params.playbackRate=Number(playback.playbackRate);
      if(playback.playbackEase)params.playbackEase=playback.playbackEase;
      if(mode==='previewSafe'){{
        params.onUpdate=function(self){{emitPreviewState(interaction,self,'update');}};
        params.onPause=function(self){{emitPreviewState(interaction,self,'pause');}};
        params.onComplete=function(self){{emitPreviewState(interaction,self,'complete');}};
      }}
      return params;
    }}
    function scrollAutoplay(interaction,trigger,mode){{
      var config=interaction.trigger||{{}};
      var params={{target:trigger||first(resolveTarget(interaction.triggerTarget,null)),enter:config.start,leave:config.end,debug:false}};
      params.repeat=config.once===false;
      params.sync=mode==='scrollTrigger'?'restart':Number(config.smoothMs)>0?Number(config.smoothMs):true;
      return onScroll(params);
    }}
    function buildTimeline(interaction,trigger,mode,stack,lifecycleId){{
      stack=stack||{{}};
      lifecycleId=lifecycleId||interaction.id;
      if(stack[interaction.id]){{report(interaction,new Error('Referință nested circulară'));return null;}}
      stack[interaction.id]=true;
      var autoplay=mode==='scrollScrub'||mode==='scrollTrigger'
        ?scrollAutoplay(interaction,trigger,mode)
        :false;
      var timeline=createTimeline(playbackParameters(interaction,autoplay,mode));
      if(!timeline){{delete stack[interaction.id];return null;}}
      (interaction.markers||[]).forEach(function(marker){{
        if(timeline.label)timeline.label(marker.name,timelinePosition(interaction,marker.at));
      }});
      (interaction.actions||[]).forEach(function(action){{
        if(!action||action.enabled===false)return;
        var position=timelinePosition(interaction,action.start);
        var safeMode=mode==='previewSafe'||mode==='scrollScrub';
        if(action.type==='animate'&&timeline.add){{
          var compiled=animationParameters(interaction,action,resolveTarget(action.target,trigger));
          timeline.add(compiled.targets,compiled.params,position);
        }}else if(action.type==='set')setAction(timeline,interaction,action,trigger,!safeMode,lifecycleId);
        else if(action.type==='media'&&!safeMode)mediaAction(timeline,interaction,action,trigger,lifecycleId);
        else if(action.type==='call'&&!safeMode&&timeline.call){{
          var callback=compileCall(action.code,action);
          var callbackCleanups=[];
          var cleanupCallback=function(){{
            callbackCleanups.splice(0).reverse().forEach(function(cleanup){{try{{dispose(cleanup);}}catch(error){{}}}});
          }};
          trackEffectCleanup(lifecycleId,cleanupCallback);
          timeline.call(function(){{
            if(timeline.backwards){{cleanupCallback();return;}}
            var cleanup=callback(timeline,anime,registry,trigger);
            if(cleanup)callbackCleanups.push(cleanup);
          }},position);
        }}else if(action.type==='nested'&&timeline.sync){{
          var nested=(documentConfig.interactions||[]).find(function(candidate){{return candidate.id===action.interactionId;}});
          if(nested){{
            var nestedTimeline=buildTimeline(
              nested,
              trigger,
              safeMode?mode:'manual',
              stack,
              lifecycleId
            );
            if(nestedTimeline){{
              var desiredDuration=timelineDuration(interaction,action.duration);
              if(desiredDuration>0&&nestedTimeline.stretch)nestedTimeline.stretch(desiredDuration);
              timeline.sync(nestedTimeline,position);
            }}
          }}
        }}
      }});
      delete stack[interaction.id];
      return timeline;
    }}
    function instanceKey(interaction,trigger){{
      if(!trigger)return interaction.id;
      if(!instanceIds.has(trigger))instanceIds.set(trigger,nextInstanceId++);
      return interaction.id+'@'+instanceIds.get(trigger);
    }}
    function remember(interaction,trigger,timeline){{
      var key=instanceKey(interaction,trigger);
      registry.instances[key]=timeline;
      if(!registry.instances[interaction.id])registry.instances[interaction.id]=timeline;
      return timeline;
    }}
    function instance(interaction,trigger,mode){{
      var key=instanceKey(interaction,trigger);
      return registry.instances[key]||remember(interaction,trigger,buildTimeline(interaction,trigger,mode||'manual'));
    }}
    function control(timeline,command,interaction){{
      if(!timeline||command==='none')return;
      if(command==='restart'&&timeline.restart)timeline.restart();
      else if(command==='play'&&timeline.play)timeline.play();
      else if(command==='pause'&&timeline.pause)timeline.pause();
      else if(command==='reverse'&&timeline.reverse)timeline.reverse();
      else if(command==='reset'&&timeline.seek){{if(timeline.pause)timeline.pause();timeline.seek(0);}}
      else if(command==='toggle'){{if(timeline.paused&&timeline.play)timeline.play();else if(timeline.pause)timeline.pause();}}
      else if(timeline.restart)timeline.restart();
      if(interaction&&reducedMode(interaction)==='skipToEnd'&&timeline.seek){{
        timeline.seek(Number(timeline.iterationDuration)||naturalDuration(interaction));
      }}
    }}
    function mediaMatches(interaction,scope){{
      var queries=(interaction.conditions&&interaction.conditions.mediaQueries||[]).filter(function(condition){{return condition.enabled!==false&&condition.query;}});
      return !queries.length||queries.some(function(condition){{
        if(scope&&scope.matches&&condition.id in scope.matches)return scope.matches[condition.id];
        return !window.matchMedia||window.matchMedia(condition.query).matches;
      }});
    }}
    function reducedMode(interaction,scope){{
      var matches=scope&&scope.matches&&scope.matches.reduceMotion;
      if(matches===undefined)matches=window.matchMedia&&window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      if(!matches)return'none';
      return interaction.conditions&&interaction.conditions.reducedMotion||'reduce';
    }}
    function listen(node,event,handler,options,cleanups){{
      node.addEventListener(event,handler,options);
      (cleanups||registry.cleanups).push(function(){{node.removeEventListener(event,handler,options);}});
    }}
    function dispose(item){{
      if(!item)return;
      if(typeof item==='function')item();
      else if(item.revert)item.revert();
      else if(item.cancel)item.cancel();
      else if(item.destroy)item.destroy();
    }}
    function trackEffectCleanup(interactionId,cleanup){{
      registry.effectCleanups.push({{interactionId:interactionId,cleanup:cleanup}});
    }}
    function clearInteractionEffects(interactionId){{
      var retained=[];
      registry.effectCleanups.forEach(function(entry){{
        if(entry.interactionId!==interactionId){{retained.push(entry);return;}}
        try{{entry.cleanup();}}catch(error){{}}
      }});
      registry.effectCleanups=retained;
    }}
    function triggerNodes(interaction){{
      var nodes=resolveTarget(interaction.triggerTarget,null);
      if(!nodes.length&&(interaction.triggerTarget.kind==='document'||interaction.trigger.type==='load'))return[document];
      return nodes;
    }}
    function clearInteractionInstances(interactionId){{
      clearInteractionEffects(interactionId);
      var disposed=[];
      Object.keys(registry.instances).forEach(function(key){{
        if(key!==interactionId&&key.indexOf(interactionId+'@')!==0)return;
        var item=registry.instances[key];
        if(item&&disposed.indexOf(item)<0){{
          disposed.push(item);
          try{{dispose(item);}}catch(error){{}}
        }}
        delete registry.instances[key];
      }});
      registry.splitters=registry.splitters.filter(function(entry){{
        if(entry.interactionId!==interactionId)return true;
        try{{if(entry.instance&&entry.instance.revert)entry.instance.revert();}}catch(error){{}}
        return false;
      }});
    }}
    function installInteraction(interaction,scope){{
      var cleanups=[];
      var cleanup=function(){{
        cleanups.splice(0).forEach(function(dispose){{try{{dispose();}}catch(error){{}}}});
        clearInteractionInstances(interaction.id);
      }};
      if(!interaction||interaction.enabled===false||!mediaMatches(interaction,scope))return cleanup;
      var reduced=reducedMode(interaction,scope);
      if(reduced==='disable')return cleanup;
      var trigger=interaction.trigger||{{type:'load'}};
      var nodes=triggerNodes(interaction);
      if(trigger.type==='load'){{
        var loadNode=first(nodes)||null;
        var run=function(){{control(instance(interaction,loadNode,'manual'),'restart',interaction);}};
        if(trigger.phase==='windowLoad'&&document.readyState!=='complete')listen(window,'load',run,{{once:true}},cleanups);
        else run();
      }}else if(trigger.type==='inView'){{
        var observer=new IntersectionObserver(function(entries){{
          entries.forEach(function(entry){{
            if(!entry.isIntersecting)return;
            control(instance(interaction,entry.target,'manual'),'restart',interaction);
            if(trigger.once!==false)observer.unobserve(entry.target);
          }});
        }},{{threshold:isFinite(Number(trigger.threshold))?Number(trigger.threshold):0.15}});
        nodes.forEach(function(node){{observer.observe(node);}});
        cleanups.push(function(){{observer.disconnect();}});
      }}else if(trigger.type==='click'){{
        nodes.forEach(function(node){{
          var clicks=0;
          listen(node,'click',function(event){{
            if(trigger.preventDefault)event.preventDefault();
            clicks+=1;
            var command=clicks%2===1?trigger.firstClick:trigger.secondClick;
            control(instance(interaction,node,'manual'),command||'restart',interaction);
          }},undefined,cleanups);
        }});
      }}else if(trigger.type==='hover'){{
        nodes.forEach(function(node){{
          listen(node,'mouseenter',function(){{control(instance(interaction,node,'manual'),trigger.enter||'restart',interaction);}},undefined,cleanups);
          listen(node,'mouseleave',function(){{control(instance(interaction,node,'manual'),trigger.leave||'reverse',interaction);}},undefined,cleanups);
        }});
      }}else if(trigger.type==='scroll'&&trigger.mode==='scrub'){{
        nodes.forEach(function(node){{instance(interaction,node,'scrollScrub');}});
      }}else if(trigger.type==='scroll'){{
        nodes.forEach(function(node){{instance(interaction,node,'scrollTrigger');}});
      }}else if(trigger.type==='pointer'){{
        nodes.forEach(function(node){{
          var timeline=instance(interaction,node,'manual');
          var current=Number(trigger.rest);
          if(!isFinite(current))current=0.5;
          var targetProgress=current;
          var frame=0;
          var previousTime=0;
          var render=function(now){{
            var smoothing=Math.max(0,Number(trigger.smoothMs)||0);
            var factor=smoothing>0?1-Math.exp(-Math.max(0,now-previousTime)/smoothing):1;
            previousTime=now;
            current+=(targetProgress-current)*factor;
            if(timeline&&timeline.seek)timeline.seek(
              Math.max(0,Math.min(1,current))*(Number(timeline.iterationDuration)||naturalDuration(interaction))
            );
            if(Math.abs(targetProgress-current)>.0001)frame=requestAnimationFrame(render);
            else frame=0;
          }};
          var moveTo=function(progress){{
            targetProgress=Math.max(0,Math.min(1,progress));
            if(!frame){{previousTime=performance.now();frame=requestAnimationFrame(render);}}
          }};
          listen(node,'pointermove',function(event){{
            var rect=node.getBoundingClientRect();
            var x=(event.clientX-rect.left)/Math.max(1,rect.width);
            var y=(event.clientY-rect.top)/Math.max(1,rect.height);
            var progress=trigger.axis==='y'?y:trigger.axis==='both'?(x+y)/2:x;
            moveTo(progress);
          }},{{passive:true}},cleanups);
          listen(node,'pointerleave',function(){{moveTo(Number(trigger.rest));}},{{passive:true}},cleanups);
          cleanups.push(function(){{if(frame)cancelAnimationFrame(frame);}});
        }});
      }}else if(trigger.type==='custom'){{
        nodes.forEach(function(node){{
          listen(node,trigger.event||'pana-motion',function(event){{
            if(trigger.preventDefault)event.preventDefault();
            control(instance(interaction,node,'manual'),'restart',interaction);
          }},undefined,cleanups);
        }});
      }}
      return cleanup;
    }}
    function installBehavior(behavior){{
      if(!behavior||behavior.enabled===false)return;
      var target=resolveTarget(behavior.target,null);
      if(behavior.type==='draggable'){{
        var params={{x:behavior.axis!=='y',y:behavior.axis!=='x',cursor:behavior.cursor!==false}};
        if(behavior.container)params.container=behavior.container;
        if(Number(behavior.snap)>0)params.snap=Number(behavior.snap);
        if(isFinite(Number(behavior.friction)))params.containerFriction=Number(behavior.friction);
        registry.behaviors[behavior.id]=target.map(function(node){{return createDraggable(node,params);}});
      }}else if(behavior.type==='layout'){{
        var layoutParams={{duration:Number(behavior.durationMs)||600,ease:behavior.ease||'out(3)'}};
        if(behavior.childrenSelector)layoutParams.children=behavior.childrenSelector;
        if(behavior.properties&&behavior.properties.length)layoutParams.properties=behavior.properties;
        registry.behaviors[behavior.id]=target.map(function(node){{return createLayout(node,layoutParams);}});
      }}
    }}
    registry.updateLayout=function(id,mutate,parameters){{
      if(typeof mutate!=='function')throw new Error('updateLayout cere o funcție de mutație DOM.');
      return list(registry.behaviors[id]).map(function(layout){{
        return layout&&layout.update
          ?layout.update(function(context){{return mutate(context,layout);}},parameters||{{}})
          :null;
      }});
    }};
    function installCustom(custom){{
      if(!custom||custom.enabled===false||!custom.code)return;
      try{{registry.instances[custom.id]=new Function('anime','registry','\"use strict\";\n'+custom.code)(anime,registry);}}
      catch(error){{report(custom,error);}}
    }}
    function findInteraction(id){{
      return(documentConfig.interactions||[]).find(function(interaction){{return interaction.id===id;}})||null;
    }}
    function previewTrigger(interaction){{
      return first(triggerNodes(interaction))||null;
    }}
    function previewTimeline(interaction){{
      return interaction&&instance(interaction,previewTrigger(interaction),'previewSafe');
    }}
    registry.preview={{
      prepare:function(id){{
        var interaction=findInteraction(id);if(!interaction)return null;
        return previewTimeline(interaction);
      }},
      seek:function(id,value){{
        var interaction=findInteraction(id),timeline=previewTimeline(interaction);
        if(timeline&&timeline.pause)timeline.pause();
        if(timeline&&timeline.seek)timeline.seek(interaction.domain==='progress'?Number(value)/100*PROGRESS_DURATION:Number(value));
        emitPreviewState(interaction,timeline,'seek');
        return timeline;
      }},
      play:function(id){{
        var interaction=findInteraction(id),timeline=previewTimeline(interaction);
        control(timeline,'play',interaction);
        emitPreviewState(interaction,timeline,'play');
        return timeline;
      }},
      pause:function(id){{
        var interaction=findInteraction(id),timeline=previewTimeline(interaction);
        if(timeline&&timeline.pause)timeline.pause();
        emitPreviewState(interaction,timeline,'pause');
        return timeline;
      }},
      reverse:function(id){{
        var interaction=findInteraction(id),timeline=previewTimeline(interaction);
        control(timeline,'reverse',interaction);
        emitPreviewState(interaction,timeline,'reverse');
        return timeline;
      }},
      restart:function(id){{
        var interaction=findInteraction(id),timeline=previewTimeline(interaction);
        control(timeline,'restart',interaction);
        emitPreviewState(interaction,timeline,'restart');
        return timeline;
      }}
    }};
    function handleStudioMotionMessage(event){{
      var message=event&&event.data;
      if(!message||message.source!=='pana-studio-motion'||message.type!=='command')return;
      var command=message.command;
      var method=registry.preview&&registry.preview[command];
      if(typeof method!=='function')return;
      try{{
        if(command==='seek')method(message.interactionId,message.value);
        else method(message.interactionId);
        if(event.source&&event.source.postMessage)event.source.postMessage({{
          source:'pana-studio-motion-runtime',
          type:'command-applied',
          interactionId:message.interactionId,
          command:command,
          value:message.value
        }},'*');
      }}catch(error){{report({{id:message.interactionId||'preview'}},error);}}
    }}
    listen(window,'message',handleStudioMotionMessage);
    registry.destroy=function(){{
      registry.cleanups.splice(0).forEach(function(cleanup){{try{{cleanup();}}catch(error){{}}}});
      Object.keys(registry.scopes).forEach(function(id){{try{{dispose(registry.scopes[id]);}}catch(error){{}}}});
      registry.effectCleanups.splice(0).reverse().forEach(function(entry){{try{{entry.cleanup();}}catch(error){{}}}});
      var disposed=[];
      Object.keys(registry.instances).forEach(function(id){{
        var item=registry.instances[id];
        if(!item||disposed.indexOf(item)>=0)return;
        disposed.push(item);
        try{{dispose(item);}}catch(error){{}}
      }});
      Object.keys(registry.behaviors).forEach(function(id){{
        list(registry.behaviors[id]).forEach(function(item){{try{{dispose(item);}}catch(error){{}}}});
      }});
      registry.splitters.splice(0).forEach(function(entry){{if(entry.instance&&entry.instance.revert)entry.instance.revert();}});
      registry.instances={{}};registry.scopes={{}};registry.behaviors={{}};registry.effectCleanups=[];
    }};

    if(!previewOnly)(documentConfig.interactions||[]).forEach(function(interaction){{
      try{{
        var resolvedRoot=first(resolveTarget(interaction.triggerTarget,null));
        var root=resolvedRoot&&resolvedRoot.nodeType===1?resolvedRoot:document.documentElement;
        var mediaQueries={{reduceMotion:'(prefers-reduced-motion: reduce)'}};
        (interaction.conditions&&interaction.conditions.mediaQueries||[]).forEach(function(condition){{
          if(condition.enabled!==false&&condition.id&&condition.query)mediaQueries[condition.id]=condition.query;
        }});
        var scope=createScope({{root:root,mediaQueries:mediaQueries}});
        if(scope&&scope.add){{
          registry.scopes[interaction.id]=scope;
          scope.add(function(self){{return installInteraction(interaction,self);}});
        }}else{{
          registry.cleanups.push(installInteraction(interaction,null));
        }}
      }}catch(error){{report(interaction,error);}}
    }});
    if(!previewOnly)(documentConfig.behaviors||[]).forEach(function(behavior){{try{{installBehavior(behavior);}}catch(error){{report(behavior,error);}}}});
    if(!previewOnly)(documentConfig.customCode||[]).forEach(installCustom);
  }})();"#
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    use serde_json::json;

    use super::*;
    use crate::js::motion_model::{
        MotionAction, MotionActionRepeat, MotionAnimateAction, MotionAnimationMode,
        MotionConditions, MotionInteraction, MotionPlayback, MotionProperty,
        MotionPropertyCategory, MotionTarget, MotionTimelineDomain, MotionTrigger, MotionValue,
        MotionValueKind,
    };

    fn config() -> PageJsConfig {
        PageJsConfig {
            version: Some(2),
            blocks: Vec::new(),
            motion: Some(MotionDocument {
                interactions: vec![MotionInteraction {
                    id: "hero-load".to_string(),
                    name: "Hero load".to_string(),
                    enabled: true,
                    trigger: MotionTrigger::default(),
                    trigger_target: MotionTarget::for_data_anim("hero"),
                    conditions: MotionConditions::default(),
                    playback: MotionPlayback::default(),
                    domain: MotionTimelineDomain::Time,
                    actions: vec![MotionAction::Animate(MotionAnimateAction {
                        id: "fade".to_string(),
                        name: "Fade".to_string(),
                        enabled: true,
                        target: MotionTarget::for_data_anim("hero"),
                        start: 0.0,
                        duration: 600.0,
                        mode: MotionAnimationMode::FromTo,
                        ease: "out(3)".to_string(),
                        properties: vec![MotionProperty {
                            id: "opacity".to_string(),
                            name: "opacity".to_string(),
                            category: MotionPropertyCategory::Style,
                            from: Some(MotionValue {
                                kind: MotionValueKind::Number,
                                value: "0".to_string(),
                                unit: String::new(),
                            }),
                            to: MotionValue {
                                kind: MotionValueKind::Number,
                                value: "1".to_string(),
                                unit: String::new(),
                            },
                        }],
                        keyframes: Vec::new(),
                        stagger: None,
                        repeat: MotionActionRepeat::default(),
                        specialization: None,
                    })],
                    markers: Vec::new(),
                }],
                ..MotionDocument::default()
            }),
        }
    }

    fn browser_config() -> PageJsConfig {
        serde_json::from_value(json!({
            "version": 2,
            "motion": {
                "schemaVersion": 2,
                "animeVersion": "4.4.1",
                "interactions": [
                    {
                        "id": "preview-sequence",
                        "name": "Preview sequence",
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                        "actions": [
                            {
                                "type": "animate",
                                "id": "move",
                                "name": "Move",
                                "target": { "kind": "trigger" },
                                "duration": 200,
                                "mode": "fromTo",
                                "ease": "linear",
                                "properties": [
                                    {
                                        "id": "translate-x",
                                        "name": "translateX",
                                        "from": { "kind": "number", "value": "0", "unit": "px" },
                                        "to": { "kind": "number", "value": "100", "unit": "px" }
                                    },
                                    {
                                        "id": "opacity",
                                        "name": "opacity",
                                        "category": "style",
                                        "from": { "kind": "number", "value": "0" },
                                        "to": { "kind": "number", "value": "1" }
                                    }
                                ]
                            },
                            {
                                "type": "set",
                                "id": "state",
                                "name": "State",
                                "target": { "kind": "trigger" },
                                "start": 100,
                                "values": [
                                    {
                                        "type": "property",
                                        "name": "--motion-probe",
                                        "value": { "kind": "text", "value": "ready" }
                                    },
                                    { "type": "addClass", "name": "published-motion" },
                                    { "type": "attribute", "name": "data-motion-state", "value": "ready" }
                                ]
                            },
                            {
                                "type": "call",
                                "id": "effect",
                                "name": "Effect",
                                "start": 100,
                                "code": "window.__motionCallCount=(window.__motionCallCount||0)+1; return function(){window.__motionCallCleanup=(window.__motionCallCleanup||0)+1;};"
                            }
                        ]
                    },
                    {
                        "id": "click-sequence",
                        "name": "Click sequence",
                        "trigger": {
                            "type": "click",
                            "firstClick": "restart",
                            "secondClick": "reverse"
                        },
                        "triggerTarget": { "kind": "element", "dataAnim": "button" },
                        "actions": [{
                            "type": "animate",
                            "id": "click-fade",
                            "name": "Click fade",
                            "target": { "kind": "trigger" },
                            "duration": 80,
                            "mode": "fromTo",
                            "ease": "linear",
                            "properties": [{
                                "id": "click-opacity",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    },
                    {
                        "id": "responsive-hidden",
                        "name": "Responsive hidden",
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "element", "dataAnim": "hidden" },
                        "conditions": {
                            "mediaQueries": [{
                                "id": "impossible",
                                "query": "(min-width: 99999px)"
                            }]
                        },
                        "actions": [{
                            "type": "call",
                            "id": "hidden-call",
                            "name": "Must not run",
                            "code": "window.__hiddenMotionRan=true;"
                        }]
                    },
                    {
                        "id": "reduced-skip",
                        "name": "Reduced skip",
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "element", "dataAnim": "reduced-skip" },
                        "conditions": { "reducedMotion": "skipToEnd" },
                        "actions": [{
                            "type": "animate",
                            "id": "reduced-skip-opacity",
                            "name": "Reduced skip opacity",
                            "target": { "kind": "trigger" },
                            "duration": 200,
                            "mode": "fromTo",
                            "ease": "linear",
                            "properties": [{
                                "id": "reduced-skip-value",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    },
                    {
                        "id": "reduced-duration",
                        "name": "Reduced duration",
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "element", "dataAnim": "reduced-duration" },
                        "conditions": { "reducedMotion": "reduce" },
                        "actions": [{
                            "type": "animate",
                            "id": "reduced-duration-opacity",
                            "name": "Reduced duration opacity",
                            "target": { "kind": "trigger" },
                            "duration": 200,
                            "mode": "fromTo",
                            "ease": "linear",
                            "properties": [{
                                "id": "reduced-duration-value",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    },
                    {
                        "id": "pointer-scrub",
                        "name": "Pointer scrub",
                        "trigger": {
                            "type": "pointer",
                            "axis": "x",
                            "smoothMs": 0,
                            "rest": 0
                        },
                        "triggerTarget": { "kind": "element", "dataAnim": "pointer" },
                        "domain": "progress",
                        "actions": [{
                            "type": "animate",
                            "id": "pointer-move",
                            "name": "Pointer move",
                            "target": { "kind": "trigger" },
                            "duration": 100,
                            "mode": "fromTo",
                            "ease": "linear",
                            "properties": [{
                                "id": "pointer-x",
                                "name": "translateX",
                                "from": { "kind": "number", "value": "0", "unit": "px" },
                                "to": { "kind": "number", "value": "100", "unit": "px" }
                            }]
                        }]
                    },
                    {
                        "id": "scroll-scrub",
                        "name": "Scroll scrub",
                        "trigger": {
                            "type": "scroll",
                            "mode": "scrub",
                            "start": "bottom top",
                            "end": "top bottom",
                            "smoothMs": 100
                        },
                        "triggerTarget": { "kind": "element", "dataAnim": "scroll" },
                        "domain": "progress",
                        "actions": [{
                            "type": "animate",
                            "id": "scroll-fade",
                            "name": "Scroll fade",
                            "target": { "kind": "trigger" },
                            "duration": 100,
                            "mode": "fromTo",
                            "ease": "linear",
                            "properties": [{
                                "id": "scroll-opacity",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    }
                ],
                "behaviors": [
                    {
                        "type": "draggable",
                        "id": "drag-behavior",
                        "name": "Drag behavior",
                        "target": { "kind": "element", "dataAnim": "drag" },
                        "axis": "both",
                        "snap": 10,
                        "friction": 0.8,
                        "cursor": true
                    },
                    {
                        "type": "layout",
                        "id": "layout-behavior",
                        "name": "Layout behavior",
                        "target": { "kind": "element", "dataAnim": "layout" },
                        "childrenSelector": ".layout-item",
                        "properties": ["borderRadius"],
                        "durationMs": 120,
                        "ease": "linear"
                    }
                ],
                "customCode": [{
                    "id": "custom-cleanup",
                    "name": "Custom cleanup",
                    "code": "window.__customActive=(window.__customActive||0)+1; return function(){window.__customActive-=1;};"
                }]
            }
        }))
        .expect("valid browser fixture")
    }

    #[test]
    fn empty_document_emits_no_motion_runtime() {
        assert!(generate_motion_js(&PageJsConfig::default()).is_empty());
    }

    #[test]
    fn compiler_emits_one_interaction_timeline_and_preview_api() {
        let js = generate_motion_js(&config());
        assert!(js.contains("PANA MOTION V2"));
        assert!(js.contains("\"schemaVersion\":2"));
        assert!(js.contains("timeline.add(compiled.targets,compiled.params,position)"));
        assert!(js.contains("registry.preview"));
        assert!(js.contains("type:'state'"));
        assert!(js.contains("new WeakMap()"));
        assert!(!js.contains("Math.random"));
        assert!(!js.contains("runAnimation(item)"));
    }

    #[test]
    fn generated_runtime_is_valid_javascript_when_node_is_available() {
        let js = generate_motion_js(&browser_config());
        let mut child = match Command::new("node")
            .args(["--check", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("node could not start: {error}"),
        };
        child
            .stdin
            .as_mut()
            .expect("node stdin")
            .write_all(js.as_bytes())
            .expect("runtime stdin");
        let output = child.wait_with_output().expect("node --check");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn browser_fixture_emits_exact_runtime() {
        let js = generate_motion_js(&browser_config());
        println!(
            "PANA_MOTION_RUNTIME_JSON={}",
            serde_json::to_string(&js).expect("runtime JSON")
        );
    }

    #[test]
    fn compiler_uses_fixed_anime_version() {
        let js = generate_motion_js(&config());
        assert!(js.contains("registry.animeVersion='4.4.1'"));
    }
}
