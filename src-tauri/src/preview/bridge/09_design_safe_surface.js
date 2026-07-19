  var DESIGN_SAFE_POLICY = HTML_EDITOR_SCHEMA.designSafe;
  var DESIGN_SAFE_FORBIDDEN_ELEMENTS = DESIGN_SAFE_POLICY.forbiddenElements.reduce(function (result, name) {
    result[String(name).toLowerCase()] = true;
    return result;
  }, {});

  function designSafeElementName(element) {
    return String((element && (element.localName || element.tagName)) || "").toLowerCase();
  }

  function designSafeElementAllowedName(name) {
    return !DESIGN_SAFE_FORBIDDEN_ELEMENTS[String(name || "").toLowerCase()];
  }

  function designSafeHasActiveScriptScheme(value) {
    var scheme = "";
    var characters = String(value || "");
    for (var i = 0; i < characters.length; i += 1) {
      var character = characters[i];
      if (character === ":") return DESIGN_SAFE_POLICY.activeSchemes.indexOf(scheme) !== -1;
      if (/\s/.test(character) || character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127) continue;
      if (/^[a-z0-9+.-]$/i.test(character)) {
        scheme += character.toLowerCase();
        continue;
      }
      return false;
    }
    return false;
  }

  function designSafeAttributeAllowed(element, name, value) {
    var normalizedName = String(name || "").toLowerCase();
    if (!normalizedName) return false;
    if (DESIGN_SAFE_POLICY.forbiddenAttributes.indexOf(normalizedName) !== -1) return false;
    if (DESIGN_SAFE_POLICY.forbiddenAttributePrefixes.some(function (prefix) {
      return normalizedName.indexOf(String(prefix).toLowerCase()) === 0;
    })) return false;
    if (designSafeHasActiveScriptScheme(value)) return false;
    if (designSafeElementName(element) === "meta" && normalizedName === "http-equiv") {
      var normalizedValue = String(value || "").trim().toLowerCase();
      if (DESIGN_SAFE_POLICY.forbiddenMetaHttpEquiv.indexOf(normalizedValue) !== -1) return false;
    }
    return true;
  }

  function sanitizeDesignSafeTree(root) {
    if (!root) return root;
    var forbiddenSelector = Object.keys(DESIGN_SAFE_FORBIDDEN_ELEMENTS).join(",");
    if (root.querySelectorAll) {
      Array.prototype.forEach.call(root.querySelectorAll(forbiddenSelector), function (element) {
        if (element === INTERNAL_BRIDGE_ELEMENT) return;
        element.remove();
      });
    }

    var elements = [];
    if (root.nodeType === 1) elements.push(root);
    if (root.querySelectorAll) {
      elements = elements.concat(Array.prototype.slice.call(root.querySelectorAll("*")));
    }
    elements.forEach(function (element) {
      if (designSafeElementName(element) === "meta") {
        var httpEquiv = String(element.getAttribute("http-equiv") || "").trim().toLowerCase();
        if (httpEquiv === "refresh" || httpEquiv === "content-security-policy") {
          element.remove();
          return;
        }
      }
      Array.prototype.slice.call(element.attributes || []).forEach(function (attribute) {
        if (!designSafeAttributeAllowed(element, attribute.localName || attribute.name, attribute.value)) {
          element.removeAttribute(attribute.name);
        }
      });
    });
    return root;
  }

  function setLiveOverridesCss(css, shouldRefreshSelection) {
    setLiveStyleCss(LIVE_OVERRIDES_ID, css, shouldRefreshSelection);
  }

  function setLiveStyleCss(id, css, shouldRefreshSelection) {
    if (shouldRefreshSelection === undefined) {
      shouldRefreshSelection = true;
    }

    var styleId = String(id || LIVE_OVERRIDES_ID);
    var styleElement = document.getElementById(styleId);
    if (!styleElement) {
      styleElement = document.createElement("style");
      styleElement.id = styleId;
      styleElement.setAttribute("data-pana-internal-style", "");
      document.head.appendChild(styleElement);
    }

    styleElement.textContent = String(css || "");

    if (shouldRefreshSelection) {
      refreshSelection();
    }
  }
