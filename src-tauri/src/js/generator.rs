use super::motion::generate_motion_js;
use super::types::{PageJsConfig, PanaComponent};

pub fn generate_page_js(config: &PageJsConfig) -> String {
    let mut out = String::new();
    let interactive_config = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    out.push_str("(function () {\n");
    if let Some(metadata) = motion_metadata_comment(config) {
        out.push_str(&metadata);
        out.push('\n');
    }
    out.push_str("  var _panaStarted=false;\n");
    out.push_str("  function _panaRun(){if(_panaStarted)return;_panaStarted=true;\n");
    out.push_str("  var _an=window.anime||{},animate=_an.animate||function(){},stagger=_an.stagger||function(){return 0;},onScroll=_an.onScroll||function(){return null;};\n");
    out.push_str(&format!(
        "  var _panaInteractive=window.PanaInteractiveRuntime||null;if(_panaInteractive&&typeof _panaInteractive.installPageConfig==='function')_panaInteractive.installPageConfig({interactive_config});\n"
    ));
    out.push_str("\n");

    let motion_js = generate_motion_js(config);
    if !motion_js.is_empty() {
        out.push_str(&motion_js);
        out.push_str("\n\n");
    }

    if !config.components.is_empty() {
        out.push_str("    if(!_panaInteractive){\n");
        out.push_str("    /* === COMPONENTE === */\n\n");
        for component in &config.components {
            out.push_str(&generate_component(component));
            out.push_str("\n\n");
        }
        out.push_str("    }\n");
    }

    out.push_str("  }\n");
    out.push_str("  if (document.readyState === \"loading\") document.addEventListener(\"DOMContentLoaded\", _panaRun, { once: true }); else _panaRun();\n");
    out.push_str("})();\n");
    out
}

fn motion_metadata_comment(config: &PageJsConfig) -> Option<String> {
    if !config.has_motion_items() {
        return None;
    }
    let payload = serde_json::json!({
        "version": config.version.unwrap_or(1),
        "motion": config.motion,
    });
    let encoded = serde_json::to_string(&payload).ok()?;
    Some(format!("  // @pana-motion {}", encoded))
}

fn generate_component(component: &PanaComponent) -> String {
    let mut lines = vec![format!("    // @pana-component id={}", component.id)];
    match component.id.as_str() {
        "counter" => lines.push(generate_counter_component()),
        "accordion" => lines.push(generate_accordion_component()),
        "tabs" => lines.push(generate_tabs_component()),
        "dialog" => lines.push(generate_dialog_component()),
        "offcanvas" => lines.push(generate_offcanvas_component()),
        "nav-menu" => lines.push(generate_nav_menu_component()),
        _ => {}
    }
    lines.join("\n")
}

fn generate_counter_component() -> String {
    r#"    (function(){function initPanaCounter(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="counter"]'):[];Array.prototype.forEach.call(nodes,function(el){if(el.__panaCounterReady)return;el.__panaCounterReady=true;var target=parseInt(el.getAttribute('data-tinta')||'0',10);if(!isFinite(target))target=0;var suffix=el.getAttribute('data-sufix')||'';var duration=parseInt(el.getAttribute('data-durata')||'1800',10);if(!isFinite(duration)||duration<1)duration=1800;var started=false;function run(){if(started)return;started=true;var start=null;function frame(ts){if(start===null)start=ts;var progress=Math.min((ts-start)/duration,1);var value=Math.floor(target*progress);el.textContent=value+suffix;if(progress<1)requestAnimationFrame(frame);else el.textContent=target+suffix;}requestAnimationFrame(frame);}if('IntersectionObserver'in window){var observer=new IntersectionObserver(function(entries){entries.forEach(function(entry){if(!entry.isIntersecting)return;run();observer.unobserve(entry.target);});},{threshold:0.3});observer.observe(el);}else{run();}});}initPanaCounter(document);document.addEventListener('pana:components:init',function(event){initPanaCounter(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

fn generate_accordion_component() -> String {
    r#"    (function(){function initPanaAccordion(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="accordion"]'):[];Array.prototype.forEach.call(nodes,function(accordion){accordion.__panaAccordionReady=true;var allowMultiple=accordion.getAttribute('data-multiple')==='true';var instance=accordion.getAttribute('data-pana-instance')||'accordion';var items=accordion.querySelectorAll('[data-pana-accordion-item]');function setOpen(item,trigger,panel,open){trigger.setAttribute('aria-expanded',open?'true':'false');panel.hidden=!open;if(open)item.setAttribute('data-open','');else item.removeAttribute('data-open');}Array.prototype.forEach.call(items,function(item,index){var trigger=item.querySelector('[data-pana-accordion-trigger]');var panel=item.querySelector('[data-pana-accordion-panel]');if(!trigger||!panel)return;var triggerId=trigger.id||instance+'-trigger-'+index;var panelId=panel.id||instance+'-panel-'+index;trigger.id=triggerId;panel.id=panelId;if(trigger.tagName&&trigger.tagName.toLowerCase()==='button'&&!trigger.getAttribute('type'))trigger.setAttribute('type','button');trigger.setAttribute('aria-controls',panelId);panel.setAttribute('role','region');panel.setAttribute('aria-labelledby',triggerId);setOpen(item,trigger,panel,trigger.getAttribute('aria-expanded')==='true'||item.hasAttribute('data-open'));if(trigger.__panaAccordionReady)return;trigger.__panaAccordionReady=true;trigger.addEventListener('click',function(){var shouldOpen=trigger.getAttribute('aria-expanded')!=='true';if(shouldOpen&&!allowMultiple){Array.prototype.forEach.call(items,function(otherItem){if(otherItem===item)return;var otherTrigger=otherItem.querySelector('[data-pana-accordion-trigger]');var otherPanel=otherItem.querySelector('[data-pana-accordion-panel]');if(otherTrigger&&otherPanel)setOpen(otherItem,otherTrigger,otherPanel,false);});}setOpen(item,trigger,panel,shouldOpen);});});});}initPanaAccordion(document);document.addEventListener('pana:components:init',function(event){initPanaAccordion(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

fn generate_tabs_component() -> String {
    r#"    (function(){function initPanaTabs(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="tabs"]'):[];Array.prototype.forEach.call(nodes,function(tabs){tabs.__panaTabsReady=true;var instance=tabs.getAttribute('data-pana-instance')||'tabs';var tabNodes=tabs.querySelectorAll('[data-pana-tabs-tab]');var panelNodes=tabs.querySelectorAll('[data-pana-tabs-panel]');if(!tabNodes.length||!panelNodes.length)return;function activate(index,shouldFocus){Array.prototype.forEach.call(tabNodes,function(tab,tabIndex){var active=tabIndex===index;tab.setAttribute('aria-selected',active?'true':'false');tab.setAttribute('tabindex',active?'0':'-1');if(active&&shouldFocus&&typeof tab.focus==='function')tab.focus();});Array.prototype.forEach.call(panelNodes,function(panel,panelIndex){panel.hidden=panelIndex!==index;});}var activeIndex=0;Array.prototype.forEach.call(tabNodes,function(tab,index){var panel=panelNodes[index];if(!panel)return;var tabId=instance+'-tab-'+index;var panelId=instance+'-panel-'+index;tab.id=tabId;panel.id=panelId;if(tab.tagName&&tab.tagName.toLowerCase()==='button'&&!tab.getAttribute('type'))tab.setAttribute('type','button');tab.setAttribute('role','tab');tab.setAttribute('aria-controls',panelId);panel.setAttribute('role','tabpanel');panel.setAttribute('aria-labelledby',tabId);if(tab.getAttribute('aria-selected')==='true'||panel.hasAttribute('data-active'))activeIndex=index;if(tab.__panaTabsReady)return;tab.__panaTabsReady=true;tab.addEventListener('click',function(){activate(index,false);});tab.addEventListener('keydown',function(event){var key=event.key;if(key!=='ArrowRight'&&key!=='ArrowLeft'&&key!=='Home'&&key!=='End')return;event.preventDefault();var nextIndex=index;if(key==='ArrowRight')nextIndex=(index+1)%tabNodes.length;if(key==='ArrowLeft')nextIndex=(index-1+tabNodes.length)%tabNodes.length;if(key==='Home')nextIndex=0;if(key==='End')nextIndex=tabNodes.length-1;activate(nextIndex,true);});});activate(activeIndex,false);});}initPanaTabs(document);document.addEventListener('pana:components:init',function(event){initPanaTabs(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

fn generate_dialog_component() -> String {
    r#"    (function(){function initPanaDialog(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="dialog"]'):[];Array.prototype.forEach.call(nodes,function(dialog){dialog.__panaDialogReady=true;var instance=dialog.getAttribute('data-pana-instance')||'dialog';var openers=dialog.querySelectorAll('[data-pana-dialog-open]');var overlay=dialog.querySelector('[data-pana-dialog-overlay]');var panel=dialog.querySelector('[data-pana-dialog-panel]');var closers=dialog.querySelectorAll('[data-pana-dialog-close]');if(!overlay||!panel)return;var panelId=instance+'-panel';var title=dialog.querySelector('[data-pana-dialog-title]');panel.id=panelId;panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','true');if(!panel.getAttribute('tabindex'))panel.setAttribute('tabindex','-1');if(title){title.id=instance+'-title';panel.setAttribute('aria-labelledby',title.id);}function setExpanded(open){Array.prototype.forEach.call(openers,function(opener){opener.setAttribute('aria-expanded',open?'true':'false');opener.setAttribute('aria-controls',panelId);});}function firstFocusable(){return panel.querySelector('button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])');}function openDialog(opener){dialog.__panaDialogPreviousActive=document.activeElement instanceof HTMLElement?document.activeElement:opener||null;dialog.__panaDialogPreviousOverflow=document.body.style.overflow||'';overlay.hidden=false;document.body.style.overflow='hidden';setExpanded(true);var focusTarget=firstFocusable()||panel;if(focusTarget&&typeof focusTarget.focus==='function')focusTarget.focus();}function closeDialog(restoreFocus){overlay.hidden=true;document.body.style.overflow=dialog.__panaDialogPreviousOverflow||'';setExpanded(false);if(restoreFocus!==false){var previous=dialog.__panaDialogPreviousActive;if(previous&&typeof previous.focus==='function'&&document.contains(previous))previous.focus();}}setExpanded(!overlay.hidden);Array.prototype.forEach.call(openers,function(opener){if(opener.tagName&&opener.tagName.toLowerCase()==='button'&&!opener.getAttribute('type'))opener.setAttribute('type','button');opener.setAttribute('aria-haspopup','dialog');if(opener.__panaDialogOpenReady)return;opener.__panaDialogOpenReady=true;opener.addEventListener('click',function(){openDialog(opener);});});Array.prototype.forEach.call(closers,function(closer){if(closer.tagName&&closer.tagName.toLowerCase()==='button'&&!closer.getAttribute('type'))closer.setAttribute('type','button');if(closer.__panaDialogCloseReady)return;closer.__panaDialogCloseReady=true;closer.addEventListener('click',function(){closeDialog(true);});});if(!overlay.__panaDialogOverlayReady){overlay.__panaDialogOverlayReady=true;overlay.addEventListener('click',function(event){if(event.target===overlay)closeDialog(true);});overlay.addEventListener('keydown',function(event){if(event.key==='Escape')closeDialog(true);});}});}initPanaDialog(document);document.addEventListener('pana:components:init',function(event){initPanaDialog(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

fn generate_offcanvas_component() -> String {
    r#"    (function(){function initPanaOffcanvas(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="offcanvas"]'):[];Array.prototype.forEach.call(nodes,function(offcanvas){offcanvas.__panaOffcanvasReady=true;var instance=offcanvas.getAttribute('data-pana-instance')||'offcanvas';var openers=offcanvas.querySelectorAll('[data-pana-offcanvas-open]');var overlay=offcanvas.querySelector('[data-pana-offcanvas-overlay]');var panel=offcanvas.querySelector('[data-pana-offcanvas-panel]');var closers=offcanvas.querySelectorAll('[data-pana-offcanvas-close]');if(!overlay||!panel)return;var panelId=panel.id||instance+'-panel';var title=offcanvas.querySelector('[data-pana-offcanvas-title]');panel.id=panelId;panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','true');if(!panel.getAttribute('tabindex'))panel.setAttribute('tabindex','-1');if(title){title.id=title.id||instance+'-title';panel.setAttribute('aria-labelledby',title.id);}function setExpanded(open){Array.prototype.forEach.call(openers,function(opener){opener.setAttribute('aria-expanded',open?'true':'false');opener.setAttribute('aria-controls',panelId);});}function firstFocusable(){return panel.querySelector('button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])');}function openPanel(opener){clearTimeout(offcanvas.__panaOffcanvasCloseTimer);offcanvas.__panaOffcanvasPreviousActive=document.activeElement instanceof HTMLElement?document.activeElement:opener||null;offcanvas.__panaOffcanvasPreviousOverflow=document.body.style.overflow||'';overlay.hidden=false;document.body.style.overflow='hidden';setExpanded(true);var scheduleFrame=window.requestAnimationFrame||function(fn){setTimeout(fn,0);};scheduleFrame(function(){offcanvas.setAttribute('data-open','');var focusTarget=firstFocusable()||panel;if(focusTarget&&typeof focusTarget.focus==='function')focusTarget.focus();});}function closePanel(restoreFocus){clearTimeout(offcanvas.__panaOffcanvasCloseTimer);offcanvas.removeAttribute('data-open');document.body.style.overflow=offcanvas.__panaOffcanvasPreviousOverflow||'';setExpanded(false);offcanvas.__panaOffcanvasCloseTimer=setTimeout(function(){if(!offcanvas.hasAttribute('data-open'))overlay.hidden=true;},240);if(restoreFocus!==false){var previous=offcanvas.__panaOffcanvasPreviousActive;if(previous&&typeof previous.focus==='function'&&document.contains(previous))previous.focus();}}if(overlay.hidden)offcanvas.removeAttribute('data-open');else offcanvas.setAttribute('data-open','');setExpanded(!overlay.hidden);Array.prototype.forEach.call(openers,function(opener){if(opener.tagName&&opener.tagName.toLowerCase()==='button'&&!opener.getAttribute('type'))opener.setAttribute('type','button');opener.setAttribute('aria-haspopup','dialog');if(opener.__panaOffcanvasOpenReady)return;opener.__panaOffcanvasOpenReady=true;opener.addEventListener('click',function(){openPanel(opener);});});Array.prototype.forEach.call(closers,function(closer){if(closer.tagName&&closer.tagName.toLowerCase()==='button'&&!closer.getAttribute('type'))closer.setAttribute('type','button');if(closer.__panaOffcanvasCloseReady)return;closer.__panaOffcanvasCloseReady=true;closer.addEventListener('click',function(){closePanel(true);});});if(!overlay.__panaOffcanvasOverlayReady){overlay.__panaOffcanvasOverlayReady=true;overlay.addEventListener('click',function(event){if(event.target===overlay)closePanel(true);});overlay.addEventListener('keydown',function(event){if(event.key==='Escape')closePanel(true);});}});}initPanaOffcanvas(document);document.addEventListener('pana:components:init',function(event){initPanaOffcanvas(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

fn generate_nav_menu_component() -> String {
    r#"    (function(){function initPanaNavMenu(root){var scope=root||document;var nodes=scope.querySelectorAll?scope.querySelectorAll('[data-pana-component="nav-menu"]'):[];Array.prototype.forEach.call(nodes,function(nav){nav.__panaNavMenuReady=true;var instance=nav.getAttribute('data-pana-instance')||'nav-menu';var toggle=nav.querySelector('[data-pana-nav-menu-toggle]');var list=nav.querySelector('[data-pana-nav-menu-list]');if(!toggle||!list)return;var listId=list.id||instance+'-list';list.id=listId;toggle.setAttribute('aria-controls',listId);if(toggle.tagName&&toggle.tagName.toLowerCase()==='button'&&!toggle.getAttribute('type'))toggle.setAttribute('type','button');var media=window.matchMedia?window.matchMedia('(max-width: 720px)'):null;function isCompact(){return media?media.matches:false;}function setOpen(open){if(open)nav.setAttribute('data-open','');else nav.removeAttribute('data-open');toggle.setAttribute('aria-expanded',open?'true':'false');list.hidden=isCompact()?!open:false;}setOpen(nav.hasAttribute('data-open'));if(!toggle.__panaNavMenuToggleReady){toggle.__panaNavMenuToggleReady=true;toggle.addEventListener('click',function(){setOpen(!nav.hasAttribute('data-open'));});}Array.prototype.forEach.call(list.querySelectorAll('a[href]'),function(link){if(link.__panaNavMenuLinkReady)return;link.__panaNavMenuLinkReady=true;link.addEventListener('click',function(){if(isCompact())setOpen(false);});});if(!nav.__panaNavMenuKeyReady){nav.__panaNavMenuKeyReady=true;nav.addEventListener('keydown',function(event){if(event.key==='Escape'&&nav.hasAttribute('data-open')){setOpen(false);if(typeof toggle.focus==='function')toggle.focus();}});}if(!nav.__panaNavMenuMediaReady&&media){nav.__panaNavMenuMediaReady=true;var sync=function(){setOpen(nav.hasAttribute('data-open'));};if(typeof media.addEventListener==='function')media.addEventListener('change',sync);else if(typeof media.addListener==='function')media.addListener(sync);}});}initPanaNavMenu(document);document.addEventListener('pana:components:init',function(event){initPanaNavMenu(event.detail&&event.detail.root?event.detail.root:document);});})();"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_page_js_runs_after_dom_is_already_ready() {
        let js = generate_page_js(&PageJsConfig::default());
        assert!(js.contains("document.readyState === \"loading\""));
        assert!(js.contains("else _panaRun();"));
    }

    #[test]
    fn generated_page_js_embeds_motion_metadata_when_present() {
        let config = PageJsConfig {
            version: Some(1),
            motion: Some(serde_json::json!({
                "schemaVersion": 1,
                "animeVersion": "4.4.1",
                "items": [{ "id": "animation-a", "type": "animation" }]
            })),
            ..PageJsConfig::default()
        };
        let js = generate_page_js(&config);
        assert!(js.contains("// @pana-motion "));
        assert!(js.contains("MOTION STUDIO"));
        assert!(js.contains("\"type\":\"animation\""));
    }

    #[test]
    fn generated_counter_uses_volatile_ready_flag() {
        let js = generate_counter_component();
        assert!(js.contains("__panaCounterReady"));
        assert!(!js.contains("data-pana-component-ready"));
    }

    #[test]
    fn generated_page_js_delegates_pana_components_to_interactive_lifecycle() {
        let config = PageJsConfig {
            components: vec![PanaComponent {
                id: "accordion".to_string(),
            }],
            ..PageJsConfig::default()
        };
        let js = generate_page_js(&config);

        assert!(js.contains("window.PanaInteractiveRuntime"));
        assert!(js.contains("installPageConfig"));
        assert!(js.contains("if(!_panaInteractive)"));
        assert!(js.contains("\"id\":\"accordion\""));
    }

    #[test]
    fn generated_accordion_uses_rehydratable_runtime() {
        let js = generate_component(&PanaComponent {
            id: "accordion".to_string(),
        });
        assert!(js.contains("// @pana-component id=accordion"));
        assert!(js.contains("initPanaAccordion"));
        assert!(js.contains("__panaAccordionReady"));
        assert!(js.contains("pana:components:init"));
        assert!(js.contains("aria-expanded"));
    }

    #[test]
    fn generated_tabs_uses_rehydratable_runtime() {
        let js = generate_component(&PanaComponent {
            id: "tabs".to_string(),
        });
        assert!(js.contains("// @pana-component id=tabs"));
        assert!(js.contains("initPanaTabs"));
        assert!(js.contains("__panaTabsReady"));
        assert!(js.contains("pana:components:init"));
        assert!(js.contains("role','tab"));
        assert!(js.contains("ArrowRight"));
    }

    #[test]
    fn generated_dialog_uses_rehydratable_runtime() {
        let js = generate_component(&PanaComponent {
            id: "dialog".to_string(),
        });
        assert!(js.contains("// @pana-component id=dialog"));
        assert!(js.contains("initPanaDialog"));
        assert!(js.contains("__panaDialogReady"));
        assert!(js.contains("pana:components:init"));
        assert!(js.contains("aria-haspopup"));
        assert!(js.contains("Escape"));
    }

    #[test]
    fn generated_offcanvas_uses_rehydratable_runtime() {
        let js = generate_component(&PanaComponent {
            id: "offcanvas".to_string(),
        });
        assert!(js.contains("// @pana-component id=offcanvas"));
        assert!(js.contains("initPanaOffcanvas"));
        assert!(js.contains("__panaOffcanvasReady"));
        assert!(js.contains("pana:components:init"));
        assert!(js.contains("data-open"));
        assert!(js.contains("Escape"));
    }

    #[test]
    fn generated_nav_menu_uses_rehydratable_runtime() {
        let js = generate_component(&PanaComponent {
            id: "nav-menu".to_string(),
        });
        assert!(js.contains("// @pana-component id=nav-menu"));
        assert!(js.contains("initPanaNavMenu"));
        assert!(js.contains("__panaNavMenuReady"));
        assert!(js.contains("pana:components:init"));
        assert!(js.contains("aria-controls"));
        assert!(js.contains("matchMedia"));
        assert!(js.contains("Escape"));
    }
}
