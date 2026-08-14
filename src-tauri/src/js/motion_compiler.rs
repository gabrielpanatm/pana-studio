use std::collections::BTreeSet;

use super::motion_model::{
    MotionAction, MotionBehavior, MotionDocument, MotionInteraction, MotionSpecialization,
    MotionTrigger,
};
use crate::kernel::generated_assets::registry::anime_esm_public_module_path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MotionFeatureSet {
    pub interactions: bool,
    pub animate: bool,
    pub set: bool,
    pub media: bool,
    pub call: bool,
    pub nested: bool,
    pub stagger: bool,
    pub split_text: bool,
    pub svg: bool,
    pub load: bool,
    pub in_view: bool,
    pub click: bool,
    pub hover: bool,
    pub scroll: bool,
    pub pointer: bool,
    pub custom_trigger: bool,
    pub draggable: bool,
    pub layout: bool,
    pub custom_code: bool,
    pub arbitrary_anime_api: bool,
}

impl MotionFeatureSet {
    pub fn from_document(document: &MotionDocument) -> Self {
        let mut features = Self {
            custom_code: document
                .custom_code
                .iter()
                .any(|custom| custom.enabled && !custom.code.trim().is_empty()),
            ..Self::default()
        };
        for interaction in document
            .interactions
            .iter()
            .filter(|interaction| interaction.enabled)
        {
            let mut visiting = BTreeSet::new();
            if features.collect_interaction_actions(
                interaction,
                &document.interactions,
                &mut visiting,
            ) {
                features.interactions = true;
                match interaction.trigger {
                    MotionTrigger::Load { .. } => features.load = true,
                    MotionTrigger::InView { .. } => features.in_view = true,
                    MotionTrigger::Click { .. } => features.click = true,
                    MotionTrigger::Hover { .. } => features.hover = true,
                    MotionTrigger::Scroll { .. } => features.scroll = true,
                    MotionTrigger::Pointer { .. } => features.pointer = true,
                    MotionTrigger::Custom { .. } => features.custom_trigger = true,
                }
            }
        }
        for behavior in document.behaviors.iter().filter(|behavior| match behavior {
            MotionBehavior::Draggable(behavior) => behavior.enabled,
            MotionBehavior::Layout(behavior) => behavior.enabled,
        }) {
            match behavior {
                MotionBehavior::Draggable(_) => features.draggable = true,
                MotionBehavior::Layout(_) => features.layout = true,
            }
        }
        features.arbitrary_anime_api |= features.custom_code;
        features
    }

    fn collect_interaction_actions(
        &mut self,
        interaction: &MotionInteraction,
        interactions: &[MotionInteraction],
        visiting: &mut BTreeSet<String>,
    ) -> bool {
        if !visiting.insert(interaction.id.clone()) {
            return false;
        }
        let mut has_enabled_action = false;
        for action in interaction.actions.iter().filter(|action| action.enabled()) {
            has_enabled_action = true;
            match action {
                MotionAction::Animate(action) => {
                    self.animate = true;
                    self.stagger |= action.stagger.is_some();
                    match action.specialization {
                        Some(MotionSpecialization::SplitText { .. }) => self.split_text = true,
                        Some(
                            MotionSpecialization::SvgPath { .. }
                            | MotionSpecialization::SvgMorph { .. }
                            | MotionSpecialization::SvgDraw,
                        ) => self.svg = true,
                        None => {}
                    }
                }
                MotionAction::Set(_) => self.set = true,
                MotionAction::Media(_) => self.media = true,
                MotionAction::Call(_) => {
                    self.call = true;
                    self.arbitrary_anime_api = true;
                }
                MotionAction::Nested(nested) => {
                    self.nested = true;
                    if let Some(child) = interactions
                        .iter()
                        .find(|candidate| candidate.id == nested.interaction_id)
                    {
                        self.collect_interaction_actions(child, interactions, visiting);
                    }
                }
            }
        }
        visiting.remove(&interaction.id);
        has_enabled_action
    }

    pub fn has_runtime(&self) -> bool {
        !self.anime_entry_modules().is_empty()
    }

    pub fn anime_entry_modules(&self) -> BTreeSet<&'static str> {
        if self.arbitrary_anime_api {
            return BTreeSet::from(["index.js"]);
        }
        let mut modules = BTreeSet::new();
        if self.interactions {
            modules.insert("timeline/index.js");
        }
        if self.stagger {
            modules.insert("utils/stagger.js");
        }
        if self.scroll {
            modules.insert("events/index.js");
        }
        if self.split_text {
            modules.insert("text/index.js");
        }
        if self.svg {
            modules.insert("svg/index.js");
        }
        if self.draggable {
            modules.insert("draggable/index.js");
        }
        if self.layout {
            modules.insert("layout/index.js");
        }
        modules
    }
}

pub fn compile_motion_production_js(document: &MotionDocument) -> String {
    let features = MotionFeatureSet::from_document(document);
    let entries = features.anime_entry_modules();
    if entries.is_empty() {
        return String::new();
    }
    let payload = execution_payload(document);
    let mut output = String::from("    void Promise.all([");
    for (index, module) in entries.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("import(");
        output.push_str(
            &serde_json::to_string(&anime_esm_public_module_path(module))
                .expect("Anime.js module path must serialize"),
        );
        output.push(')');
    }
    output.push_str("]).then(function(_modules){\n");
    output.push_str(
        "      var anime={};_modules.forEach(function(module){Object.assign(anime,module);});\n",
    );
    output.push_str("      var config=");
    output.push_str(&payload);
    output.push_str(";\n");
    output.push_str(PRODUCTION_RUNTIME_BASE);
    if features.animate {
        output.push_str(PRODUCTION_ANIMATE_HELPERS);
        if features.stagger {
            output.push_str(PRODUCTION_STAGGER_HELPER);
        } else {
            output.push_str("\n      function applyStagger(){}\n");
        }
        if features.split_text || features.svg {
            output.push_str(PRODUCTION_SPECIALIZATION_HELPER);
        } else {
            output.push_str(
                "\n      function specialize(_interaction,_action,nodes){return nodes;}\n",
            );
        }
    }
    if features.set {
        output.push_str(PRODUCTION_SET_ACTION);
    }
    if features.media {
        output.push_str(PRODUCTION_MEDIA_ACTION);
    }
    if features.call {
        output.push_str(PRODUCTION_CALL_ACTION);
    }
    output.push_str(&build_timeline_source(&features));
    output.push_str(&install_interaction_source(&features));
    if features.draggable || features.layout {
        output.push_str(&install_behaviors_source(&features));
    }
    if features.custom_code {
        output.push_str(PRODUCTION_CUSTOM_CODE);
    }
    output.push_str(PRODUCTION_BOOT);
    output.push_str("    }).catch(function(error){if(window.console&&console.error)console.error('[Pană Motion]',error);});");
    output
}

pub(super) fn execution_payload(document: &MotionDocument) -> String {
    let mut payload = serde_json::to_value(document)
        .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
    if let Some(object) = payload.as_object_mut() {
        object.remove("schemaVersion");
        object.remove("animeVersion");
        remove_editorial_names(object.get_mut("interactions"));
        remove_editorial_names(object.get_mut("behaviors"));
        remove_editorial_names(object.get_mut("customCode"));
    }
    prune_execution_value(&mut payload);
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn remove_editorial_names(value: Option<&mut serde_json::Value>) {
    let Some(items) = value.and_then(serde_json::Value::as_array_mut) else {
        return;
    };
    for item in items {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        object.remove("name");
        if let Some(actions) = object
            .get_mut("actions")
            .and_then(serde_json::Value::as_array_mut)
        {
            for action in actions {
                if let Some(action) = action.as_object_mut() {
                    action.remove("name");
                }
            }
        }
    }
}

fn prune_execution_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => items.iter_mut().for_each(prune_execution_value),
        serde_json::Value::Object(object) => {
            object.values_mut().for_each(prune_execution_value);
            object.retain(|key, value| {
                if key == "enabled" && value == &serde_json::Value::Bool(true) {
                    return false;
                }
                !value.is_null()
                    && !matches!(value, serde_json::Value::String(text) if text.is_empty())
                    && !matches!(value, serde_json::Value::Array(items) if items.is_empty())
                    && !matches!(value, serde_json::Value::Object(map) if map.is_empty())
            });
        }
        _ => {}
    }
}

fn build_timeline_source(features: &MotionFeatureSet) -> String {
    let mut source = String::from(
        r#"
      function buildTimeline(interaction,trigger,reduced,stack){
        stack=stack||{};
        if(stack[interaction.id]){report(interaction,new Error('Referință nested circulară'));return null;}
        stack[interaction.id]=true;
        var timeline=anime.createTimeline(playbackParameters(interaction,false,reduced));
        if(!timeline){delete stack[interaction.id];return null;}
        (interaction.markers||[]).forEach(function(marker){if(timeline.label)timeline.label(marker.name,timelinePosition(interaction,marker.at));});
        (interaction.actions||[]).forEach(function(action){
          if(!action||action.enabled===false)return;
          var position=timelinePosition(interaction,action.start);
"#,
    );
    let mut first = true;
    let mut branch = |condition: &str, body: &str| {
        source.push_str(if first {
            "          if("
        } else {
            "          else if("
        });
        first = false;
        source.push_str(condition);
        source.push_str("){\n            ");
        source.push_str(body);
        source.push_str("\n          }\n");
    };
    if features.animate {
        branch(
            "action.type==='animate'&&timeline.add",
            "var compiled=animationParameters(interaction,action,resolveTarget(action.target,trigger),reduced);\n            timeline.add(compiled.targets,compiled.params,position);",
        );
    }
    if features.set {
        branch(
            "action.type==='set'",
            "installSetAction(timeline,interaction,action,trigger);",
        );
    }
    if features.media {
        branch(
            "action.type==='media'",
            "installMediaAction(timeline,interaction,action,trigger);",
        );
    }
    if features.call {
        branch(
            "action.type==='call'&&timeline.call",
            "installCallAction(timeline,interaction,action,trigger);",
        );
    }
    if features.nested {
        branch(
            "action.type==='nested'&&timeline.sync",
            "var nested=(config.interactions||[]).find(function(candidate){return candidate.id===action.interactionId;});\n            if(nested){var child=buildTimeline(nested,trigger,reduced,stack);if(child){var desired=timelineDuration(interaction,action.duration,reduced);if(desired>0&&child.stretch)child.stretch(desired);timeline.sync(child,position);}}",
        );
    }
    source.push_str(
        r#"        });
        delete stack[interaction.id];
        return timeline;
      }
"#,
    );
    source
}

fn install_interaction_source(features: &MotionFeatureSet) -> String {
    let mut source = String::from(
        r#"
      function installInteraction(interaction){
        if(!interaction||interaction.enabled===false||!mediaMatches(interaction))return function(){};
        var reduced=reducedMode(interaction);
        if(reduced==='disable')return function(){};
        var trigger=interaction.trigger||{type:'load'};
        var nodes=triggerNodes(interaction);
        var local=[];
"#,
    );
    let mut first = true;
    let mut branch = |condition: &str, body: &str| {
        source.push_str(if first {
            "        if("
        } else {
            "        else if("
        });
        first = false;
        source.push_str(condition);
        source.push_str("){\n          ");
        source.push_str(body);
        source.push_str("\n        }\n");
    };
    if features.load {
        branch(
            "trigger.type==='load'",
            "var loadNode=first(nodes)||null;var run=function(){control(instance(interaction,loadNode,reduced),'restart',interaction,reduced);};\n          if(trigger.phase==='windowLoad'&&document.readyState!=='complete')listen(window,'load',run,{once:true},local);else run();",
        );
    }
    if features.in_view {
        branch(
            "trigger.type==='inView'",
            "var observer=new IntersectionObserver(function(entries){entries.forEach(function(entry){if(!entry.isIntersecting)return;control(instance(interaction,entry.target,reduced),'restart',interaction,reduced);if(trigger.once!==false)observer.unobserve(entry.target);});},{threshold:isFinite(Number(trigger.threshold))?Number(trigger.threshold):0.15});\n          nodes.forEach(function(node){observer.observe(node);});local.push(function(){observer.disconnect();});",
        );
    }
    if features.click {
        branch(
            "trigger.type==='click'",
            "nodes.forEach(function(node){var clicks=0;listen(node,'click',function(event){if(trigger.preventDefault)event.preventDefault();clicks+=1;var command=clicks%2===1?trigger.firstClick:trigger.secondClick;control(instance(interaction,node,reduced),command||'restart',interaction,reduced);},undefined,local);});",
        );
    }
    if features.hover {
        branch(
            "trigger.type==='hover'",
            "nodes.forEach(function(node){listen(node,'mouseenter',function(){control(instance(interaction,node,reduced),trigger.enter||'restart',interaction,reduced);},undefined,local);listen(node,'mouseleave',function(){control(instance(interaction,node,reduced),trigger.leave||'reverse',interaction,reduced);},undefined,local);});",
        );
    }
    if features.scroll {
        branch(
            "trigger.type==='scroll'",
            "nodes.forEach(function(node){var autoplay=anime.onScroll({target:node,enter:trigger.start,leave:trigger.end,debug:false,repeat:trigger.once===false,sync:trigger.mode==='scrub'?'restart':Number(trigger.smoothMs)>0?Number(trigger.smoothMs):true});var timeline=anime.createTimeline(playbackParameters(interaction,autoplay,reduced));buildActionsIntoTimeline(timeline,interaction,node,reduced);remember(interaction,node,timeline);});",
        );
    }
    if features.pointer {
        branch(
            "trigger.type==='pointer'",
            "nodes.forEach(function(node){var timeline=instance(interaction,node,reduced);var current=Number(trigger.rest);if(!isFinite(current))current=.5;var target=current,frame=0,previous=0;var render=function(now){var smoothing=Math.max(0,Number(trigger.smoothMs)||0);var factor=smoothing>0?1-Math.exp(-Math.max(0,now-previous)/smoothing):1;previous=now;current+=(target-current)*factor;if(timeline&&timeline.seek)timeline.seek(Math.max(0,Math.min(1,current))*(Number(timeline.iterationDuration)||naturalDuration(interaction,reduced)));if(Math.abs(target-current)>.0001)frame=requestAnimationFrame(render);else frame=0;};var move=function(value){target=Math.max(0,Math.min(1,value));if(!frame){previous=performance.now();frame=requestAnimationFrame(render);}};listen(node,'pointermove',function(event){var rect=node.getBoundingClientRect(),x=(event.clientX-rect.left)/Math.max(1,rect.width),y=(event.clientY-rect.top)/Math.max(1,rect.height);move(trigger.axis==='y'?y:trigger.axis==='both'?(x+y)/2:x);},{passive:true},local);listen(node,'pointerleave',function(){move(Number(trigger.rest));},{passive:true},local);local.push(function(){if(frame)cancelAnimationFrame(frame);});});",
        );
    }
    if features.custom_trigger {
        branch(
            "trigger.type==='custom'",
            "nodes.forEach(function(node){listen(node,trigger.event||'pana-motion',function(event){if(trigger.preventDefault)event.preventDefault();control(instance(interaction,node,reduced),'restart',interaction,reduced);},undefined,local);});",
        );
    }
    source.push_str(
        r#"        return function(){local.splice(0).forEach(dispose);clearInteraction(interaction.id);};
      }
"#,
    );
    if features.scroll {
        source.push_str(PRODUCTION_SCROLL_TIMELINE_HELPER);
    }
    source
}

fn install_behaviors_source(features: &MotionFeatureSet) -> String {
    let mut source = String::from("\n      function installBehaviors(){\n");
    if features.draggable {
        source.push_str(
            "        (config.behaviors||[]).filter(function(item){return item.type==='draggable'&&item.enabled!==false;}).forEach(function(item){var params={x:item.axis!=='y',y:item.axis!=='x',cursor:item.cursor!==false};if(item.container)params.container=item.container;if(Number(item.snap)>0)params.snap=Number(item.snap);if(isFinite(Number(item.friction)))params.containerFriction=Number(item.friction);registry.behaviors[item.id]=resolveTarget(item.target,null).map(function(node){return anime.createDraggable(node,params);});});\n",
        );
    }
    if features.layout {
        source.push_str(
            "        (config.behaviors||[]).filter(function(item){return item.type==='layout'&&item.enabled!==false;}).forEach(function(item){var params={duration:Number(item.durationMs)||600,ease:item.ease||'out(3)'};if(item.childrenSelector)params.children=item.childrenSelector;if(item.properties&&item.properties.length)params.properties=item.properties;registry.behaviors[item.id]=resolveTarget(item.target,null).map(function(node){return anime.createLayout(node,params);});});\n",
        );
        source.push_str("        registry.updateLayout=function(id,mutate,parameters){if(typeof mutate!=='function')throw new Error('updateLayout cere o funcție de mutație DOM.');return list(registry.behaviors[id]).map(function(layout){return layout&&layout.update?layout.update(function(context){return mutate(context,layout);},parameters||{}):null;});};\n");
    }
    source.push_str("      }\n");
    source
}

const PRODUCTION_RUNTIME_BASE: &str = r#"
      var registry={instances:{},behaviors:{},errors:[],cleanups:[],effects:[],splitters:[]};
      var instanceIds=new WeakMap(),nextInstanceId=1;
      function report(owner,error){var diagnostic={id:owner&&owner.id||'',message:error&&error.message?error.message:String(error)};registry.errors.push(diagnostic);if(window.console&&console.warn)console.warn('[Pană Motion]',diagnostic.id,error);}
      function list(value){if(!value)return[];if(Array.isArray(value))return value;if(typeof value.length==='number'&&typeof value!=='string')return Array.prototype.slice.call(value);return[value];}
      function first(value){return list(value)[0]||null;}
      function cssEscape(value){if(window.CSS&&typeof window.CSS.escape==='function')return window.CSS.escape(String(value));return String(value).replace(/["\\]/g,'\\$&');}
      function query(selector,root){if(!selector)return[];try{return list((root||document).querySelectorAll(selector));}catch(error){report({id:'target'},error);return[];}}
      function targetSelector(ref){if(!ref)return'';if(ref.kind==='element'&&ref.dataAnim)return'[data-anim="'+cssEscape(ref.dataAnim)+'"]';return ref.selector||'';}
      function resolveTarget(ref,trigger){
        if(!ref)return[];if(ref.kind==='trigger')return trigger?[trigger]:[];if(ref.kind==='viewport')return[window];if(ref.kind==='document')return[document];
        var nodes=[];
        if(ref.kind==='relative'&&trigger){var selector=ref.selector||'*';if(ref.relation==='children')nodes=list(trigger.children).filter(function(node){return node.matches&&node.matches(selector);});else if(ref.relation==='descendants')nodes=query(selector,trigger);else if(ref.relation==='parent')nodes=trigger.parentElement&&trigger.parentElement.matches(selector)?[trigger.parentElement]:[];else if(ref.relation==='ancestors'){var node=trigger.parentElement;while(node){if(node.matches&&node.matches(selector))nodes.push(node);node=node.parentElement;}}else if(ref.relation==='siblings')nodes=trigger.parentElement?list(trigger.parentElement.children).filter(function(node){return node!==trigger&&node.matches&&node.matches(selector);}):[];else if(ref.relation==='nextSibling')nodes=trigger.nextElementSibling&&trigger.nextElementSibling.matches(selector)?[trigger.nextElementSibling]:[];else if(ref.relation==='previousSibling')nodes=trigger.previousElementSibling&&trigger.previousElementSibling.matches(selector)?[trigger.previousElementSibling]:[];else nodes=trigger.matches&&trigger.matches(selector)?[trigger]:[];}else nodes=query(targetSelector(ref),document);
        return ref.scope==='first'?(nodes.length?[nodes[0]]:[]):nodes;
      }
      function readValue(value){if(!value)return'';var raw=value.value==null?'':String(value.value),unit=value.unit||'';if(value.kind==='cssVariable')return raw.indexOf('var(')===0?raw:'var('+raw+')';if(value.kind==='number'&&!unit&&raw.trim()!==''&&isFinite(Number(raw)))return Number(raw);return raw+unit;}
      function timelinePosition(interaction,value){return interaction.domain==='progress'?Number(value||0)/100*1000:Number(value||0);}
      function timelineDuration(interaction,value,reduced){var duration=interaction.domain==='progress'?Number(value||0)/100*1000:Number(value||0);return interaction.domain!=='progress'&&reduced==='reduce'?duration*.2:duration;}
      function naturalDuration(interaction,reduced){var max=0;(interaction.actions||[]).forEach(function(action){var duration=timelineDuration(interaction,action.duration,reduced);if(action.type==='animate'&&action.repeat&&!action.repeat.infinite){var repeats=Math.max(0,Number(action.repeat.count)||0);duration=duration*(repeats+1)+timelineDuration(interaction,action.repeat.delayMs,reduced)*repeats;}max=Math.max(max,timelinePosition(interaction,action.start)+duration);});return interaction.domain==='progress'?1000:Math.max(1,max);}
      function playbackParameters(interaction,autoplay,reduced){var playback=interaction.playback||{},params={autoplay:autoplay||false,delay:timelineDuration(interaction,playback.delayMs,reduced),loop:playback.infinite?true:Number(playback.repeat)||0,loopDelay:timelineDuration(interaction,playback.loopDelayMs,reduced),alternate:!!playback.alternate,reversed:!!playback.reversed,playbackRate:Number(playback.playbackRate)||1};if(playback.playbackEase)params.playbackEase=playback.playbackEase;return params;}
      function mediaMatches(interaction){var conditions=(interaction.conditions&&interaction.conditions.mediaQueries||[]).filter(function(item){return item.enabled!==false&&item.query;});return !conditions.length||conditions.some(function(item){return !window.matchMedia||window.matchMedia(item.query).matches;});}
      function reducedMode(interaction){var matches=window.matchMedia&&window.matchMedia('(prefers-reduced-motion: reduce)').matches;if(!matches)return'none';return interaction.conditions&&interaction.conditions.reducedMotion||'reduce';}
      function listen(node,event,handler,options,cleanups){node.addEventListener(event,handler,options);(cleanups||registry.cleanups).push(function(){node.removeEventListener(event,handler,options);});}
      function dispose(item){if(!item)return;if(typeof item==='function')item();else if(item.revert)item.revert();else if(item.cancel)item.cancel();else if(item.destroy)item.destroy();}
      function triggerNodes(interaction){var nodes=resolveTarget(interaction.triggerTarget,null);if(!nodes.length&&(interaction.triggerTarget.kind==='document'||interaction.trigger.type==='load'))return[document];return nodes;}
      function instanceKey(interaction,trigger){if(!trigger)return interaction.id;if(!instanceIds.has(trigger))instanceIds.set(trigger,nextInstanceId++);return interaction.id+'@'+instanceIds.get(trigger);}
      function remember(interaction,trigger,timeline){registry.instances[instanceKey(interaction,trigger)]=timeline;return timeline;}
      function instance(interaction,trigger,reduced){var key=instanceKey(interaction,trigger);return registry.instances[key]||remember(interaction,trigger,buildTimeline(interaction,trigger,reduced));}
      function control(timeline,command,interaction,reduced){if(!timeline||command==='none')return;if(command==='restart'&&timeline.restart)timeline.restart();else if(command==='play'&&timeline.play)timeline.play();else if(command==='pause'&&timeline.pause)timeline.pause();else if(command==='reverse'&&timeline.reverse)timeline.reverse();else if(command==='reset'&&timeline.seek){if(timeline.pause)timeline.pause();timeline.seek(0);}else if(command==='toggle'){if(timeline.paused&&timeline.play)timeline.play();else if(timeline.pause)timeline.pause();}else if(timeline.restart)timeline.restart();if(reduced==='skipToEnd'&&timeline.seek)timeline.seek(Number(timeline.iterationDuration)||naturalDuration(interaction,reduced));}
      function clearInteraction(id){Object.keys(registry.instances).forEach(function(key){if(key!==id&&key.indexOf(id+'@')!==0)return;dispose(registry.instances[key]);delete registry.instances[key];});registry.effects=registry.effects.filter(function(entry){if(entry.id!==id)return true;try{entry.cleanup();}catch(error){}return false;});}
"#;

const PRODUCTION_ANIMATE_HELPERS: &str = r#"
      function propertyValue(property,mode){var to=readValue(property.to),from=property.from?readValue(property.from):undefined;if(mode==='fromTo'&&from!==undefined)return[from,to];if(mode==='from'&&from!==undefined)return{from:from};return to;}
      function actionProperties(action){var params={};(action.properties||[]).forEach(function(property){if(property&&property.name)params[property.name]=propertyValue(property,action.mode);});return params;}
      function actionKeyframes(action){if(!action.keyframes||!action.keyframes.length)return null;var frames={};action.keyframes.forEach(function(frame){var values={};(frame.properties||[]).forEach(function(property){if(property&&property.name)values[property.name]=propertyValue(property,'to');});if(frame.ease)values.ease=frame.ease;frames[String(Math.max(0,Math.min(100,frame.offset)))+'%']=values;});return frames;}
      function animationParameters(interaction,action,nodes,reduced){var params=actionProperties(action);params.duration=timelineDuration(interaction,action.duration,reduced);if(action.ease)params.ease=action.ease;var frames=actionKeyframes(action);if(frames)params.keyframes=frames;applyStagger(params,interaction,action,reduced);if(action.repeat){if(action.repeat.infinite)params.loop=true;else if(Number(action.repeat.count)>0)params.loop=Number(action.repeat.count);if(action.repeat.alternate)params.alternate=true;if(Number(action.repeat.delayMs)>0)params.loopDelay=timelineDuration(interaction,action.repeat.delayMs,reduced);}var specialized=specialize(interaction,action,nodes);if(specialized&&specialized.motionPath)Object.keys(specialized.motionPath).forEach(function(key){params[key]=specialized.motionPath[key];});if(specialized&&specialized.morph)params.d=specialized.morph;return{targets:specialized&&specialized.nodes||specialized,params:params};}
"#;

const PRODUCTION_STAGGER_HELPER: &str = r#"
      function applyStagger(params,interaction,action,reduced){if(!action.stagger)return;var options={};if(action.stagger.from)options.from=action.stagger.from;if(action.stagger.reversed)options.reversed=true;if(action.stagger.ease)options.ease=action.stagger.ease;if(action.stagger.mode==='total')options.total=timelineDuration(interaction,action.stagger.amount,reduced);params.delay=anime.stagger(action.stagger.mode==='total'?0:timelineDuration(interaction,action.stagger.amount,reduced),options);}
"#;

const PRODUCTION_SPECIALIZATION_HELPER: &str = r#"
      function specialize(interaction,action,nodes){var spec=action.specialization;if(!spec)return nodes;if(spec.type==='splitText'&&anime.splitText){var fragments=[];nodes.forEach(function(node){var split=anime.splitText(node,{lines:spec.mode==='lines',words:spec.mode==='words',chars:spec.mode==='chars',accessible:true});if(split){registry.splitters.push(split);fragments=fragments.concat(split[spec.mode]||[]);}});return fragments.length?fragments:nodes;}if(spec.type==='svgPath'&&anime.createMotionPath){var path=first(query(spec.path));var motionPath=anime.createMotionPath(path);if(motionPath&&spec.autoRotate===false)delete motionPath.rotate;return{nodes:nodes,motionPath:motionPath};}if(spec.type==='svgMorph'&&anime.morphTo)return{nodes:nodes,morph:anime.morphTo(spec.source,Number(spec.precision)||.33)};if(spec.type==='svgDraw'&&anime.createDrawable)return anime.createDrawable(nodes);return nodes;}
"#;

const PRODUCTION_SET_ACTION: &str = r#"
      function installSetAction(timeline,interaction,action,trigger){var targets=resolveTarget(action.target,trigger),properties={},side=[];(action.values||[]).forEach(function(value){if(value.type==='property')properties[value.name]=readValue(value.value);else side.push(value);});var position=timelinePosition(interaction,action.start);if(Object.keys(properties).length&&timeline.set)timeline.set(targets,properties,position);if(side.length&&timeline.call){var restorers=[],active=false,restore=function(){restorers.splice(0).reverse().forEach(function(revert){try{revert();}catch(error){}});active=false;};registry.effects.push({id:interaction.id,cleanup:restore});timeline.call(function(){if(timeline.backwards){restore();return;}if(!active){active=true;targets.forEach(function(node){side.forEach(function(value){if(value.type==='attribute'){var had=node.hasAttribute(value.name),previous=node.getAttribute(value.name);restorers.push(function(){if(had)node.setAttribute(value.name,previous);else node.removeAttribute(value.name);});}else{var hadClass=node.classList.contains(value.name);restorers.push(function(){if(hadClass)node.classList.add(value.name);else node.classList.remove(value.name);});}});});}targets.forEach(function(node){side.forEach(function(value){if(value.type==='attribute')node.setAttribute(value.name,value.value);else if(value.type==='addClass')node.classList.add(value.name);else if(value.type==='removeClass')node.classList.remove(value.name);else if(value.type==='toggleClass')node.classList.toggle(value.name);});});},position);}}
"#;

const PRODUCTION_MEDIA_ACTION: &str = r#"
      function installMediaAction(timeline,interaction,action,trigger){if(!timeline.call)return;var restorers=[],active=false,restore=function(){restorers.splice(0).reverse().forEach(function(revert){try{revert();}catch(error){}});active=false;};registry.effects.push({id:interaction.id,cleanup:restore});timeline.call(function(){if(timeline.backwards){restore();return;}resolveTarget(action.target,trigger).forEach(function(node){if(!active){var paused=node.paused,time=Number(node.currentTime)||0;restorers.push(function(){if(node.pause)node.pause();try{node.currentTime=time;}catch(error){}if(paused===false&&node.play){var resumed=node.play();if(resumed&&resumed.catch)resumed.catch(function(){});}});}if(action.command==='play'&&node.play){var result=node.play();if(result&&result.catch)result.catch(function(){});}else if(action.command==='pause'&&node.pause)node.pause();else if(action.command==='reset'){if(node.pause)node.pause();try{node.currentTime=0;}catch(error){}}else if(action.command==='toggle'){if(node.paused&&node.play)node.play();else if(node.pause)node.pause();}});active=true;},timelinePosition(interaction,action.start));}
"#;

const PRODUCTION_CALL_ACTION: &str = r#"
      function installCallAction(timeline,interaction,action,trigger){var callback;try{callback=new Function('timeline','anime','registry','trigger','"use strict";\n'+String(action.code||''));}catch(error){report(action,error);return;}var callbacks=[];var cleanup=function(){callbacks.splice(0).reverse().forEach(function(item){try{dispose(item);}catch(error){}});};registry.effects.push({id:interaction.id,cleanup:cleanup});timeline.call(function(){if(timeline.backwards){cleanup();return;}var result=callback(timeline,anime,registry,trigger);if(result)callbacks.push(result);},timelinePosition(interaction,action.start));}
"#;

const PRODUCTION_SCROLL_TIMELINE_HELPER: &str = r#"
      function buildActionsIntoTimeline(timeline,interaction,trigger,reduced){var original=anime.createTimeline;anime.createTimeline=function(){return timeline;};try{buildTimeline(interaction,trigger,reduced);}finally{anime.createTimeline=original;}}
"#;

const PRODUCTION_CUSTOM_CODE: &str = r#"
      function installCustomCode(){(config.customCode||[]).forEach(function(custom){if(!custom||custom.enabled===false||!custom.code)return;try{var result=new Function('anime','registry','"use strict";\n'+custom.code)(anime,registry);if(result)registry.cleanups.push(result);}catch(error){report(custom,error);}});}
"#;

const PRODUCTION_BOOT: &str = r#"
      (config.interactions||[]).forEach(function(interaction){try{registry.cleanups.push(installInteraction(interaction));}catch(error){report(interaction,error);}});
      if(typeof installBehaviors==='function')try{installBehaviors();}catch(error){report({id:'behaviors'},error);}
      if(typeof installCustomCode==='function')installCustomCode();
      function destroy(){registry.cleanups.splice(0).forEach(function(cleanup){try{dispose(cleanup);}catch(error){}});Object.keys(registry.instances).forEach(function(key){try{dispose(registry.instances[key]);}catch(error){}});Object.keys(registry.behaviors).forEach(function(key){list(registry.behaviors[key]).forEach(function(item){try{dispose(item);}catch(error){}});});registry.effects.splice(0).reverse().forEach(function(entry){try{entry.cleanup();}catch(error){}});registry.splitters.splice(0).forEach(function(split){try{if(split&&split.revert)split.revert();}catch(error){}});}
      window.addEventListener('pagehide',destroy,{once:true});
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::{MotionRuntimeContract, PageJsConfig};

    fn simple_document() -> MotionDocument {
        PageJsConfig {
            motion: Some(
                MotionDocument::from_value(serde_json::json!({
                    "schemaVersion": 2,
                    "animeVersion": MotionRuntimeContract::current().anime_version,
                    "interactions": [{
                        "id": "hero",
                        "trigger": { "type": "inView", "once": true },
                        "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                        "actions": [{
                            "type": "animate",
                            "id": "fade",
                            "target": { "kind": "element", "dataAnim": "hero" },
                            "duration": 600,
                            "properties": [{
                                "id": "opacity",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    }]
                }))
                .unwrap(),
            ),
        }
        .motion
        .unwrap()
    }

    #[test]
    fn simple_fade_selects_only_the_timeline_module_and_required_branches() {
        let document = simple_document();
        let features = MotionFeatureSet::from_document(&document);
        assert_eq!(
            features.anime_entry_modules(),
            BTreeSet::from(["timeline/index.js"])
        );
        let source = compile_motion_production_js(&document);
        assert!(source.contains("timeline/index.js"));
        assert!(source.contains("IntersectionObserver"));
        assert!(!source.contains("PanaMotionRuntime"));
        assert!(!source.contains("postMessage"));
        assert!(!source.contains("createDraggable"));
        assert!(!source.contains("pointermove"));
        assert!(
            source.len() < 14 * 1024,
            "simple Motion output was {} bytes",
            source.len()
        );
    }

    #[test]
    fn disabled_roots_emit_nothing_and_nested_roots_collect_child_actions() {
        let disabled = MotionDocument::from_value(serde_json::json!({
            "schemaVersion": 2,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "interactions": [{
                "id": "disabled",
                "enabled": false,
                "trigger": { "type": "load" },
                "triggerTarget": { "kind": "document" },
                "actions": [{
                    "type": "set",
                    "id": "disabled-set",
                    "target": { "kind": "document" },
                    "values": [{
                        "type": "attribute",
                        "name": "data-disabled",
                        "value": "true"
                    }]
                }]
            }]
        }))
        .unwrap();
        assert!(!MotionFeatureSet::from_document(&disabled).has_runtime());
        assert!(compile_motion_production_js(&disabled).is_empty());

        let nested = MotionDocument::from_value(serde_json::json!({
            "schemaVersion": 2,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "interactions": [{
                "id": "root",
                "trigger": { "type": "load" },
                "triggerTarget": { "kind": "document" },
                "actions": [{
                    "type": "nested",
                    "id": "nested-child",
                    "duration": 400,
                    "interactionId": "child"
                }]
            }, {
                "id": "child",
                "enabled": false,
                "trigger": { "type": "pointer", "axis": "x" },
                "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                "domain": "progress",
                "actions": [{
                    "type": "animate",
                    "id": "child-fade",
                    "target": { "kind": "element", "dataAnim": "hero" },
                    "duration": 100,
                    "properties": [{
                        "id": "opacity",
                        "name": "opacity",
                        "category": "style",
                        "from": { "kind": "number", "value": "0" },
                        "to": { "kind": "number", "value": "1" }
                    }]
                }]
            }]
        }))
        .unwrap();
        let features = MotionFeatureSet::from_document(&nested);
        assert!(features.interactions);
        assert!(features.nested);
        assert!(features.animate);
        assert!(features.load);
        assert!(!features.pointer);
        let source = compile_motion_production_js(&nested);
        assert!(source.contains("animationParameters"));
        assert!(source.contains("action.type==='nested'"));
    }
}
