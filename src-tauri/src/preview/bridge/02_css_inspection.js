  function stylesheetSourceLabel(href) {
    if (!href) {
      return "<style>";
    }

    try {
      var url = new URL(href, window.location.href);
      return url.pathname || href;
    } catch (error) {
      return href;
    }
  }

  function describeRuleMatch(selectorText, element) {
    var selectors = selectorText
      .split(",")
      .map(function (selector) {
        return selector.trim();
      })
      .filter(Boolean);

    var best = { kind: "descendant", score: 0 };

    selectors.forEach(function (selector) {
      var score = 0;

      try {
        if (!element.matches(selector)) {
          return;
        }
      } catch (error) {
        return;
      }

      var ids = (selector.match(/#[A-Za-z0-9_-]+/g) || []).length;
      var classes = (selector.match(/\.[A-Za-z0-9_-]+/g) || []).length;
      var attributes = (selector.match(/\[[^\]]+\]/g) || []).length;
      var pseudos = (selector.match(/:(?!:)[A-Za-z0-9_-]+/g) || []).length;
      var combinators = (selector.match(/[\s>+~]+/g) || []).length;
      var tagName = element.tagName.toLowerCase();
      var tagMatch =
        selector === tagName ||
        selector.indexOf(tagName + ".") === 0 ||
        selector.indexOf(tagName + "#") === 0;

      score += ids * 100;
      score += classes * 10;
      score += attributes * 8;
      score += pseudos * 4;
      score += tagMatch ? 6 : 0;
      score -= combinators * 3;

      var kind = "descendant";
      if (selector.charAt(0) === "#") {
        kind = "id";
      } else if (selector === tagName) {
        kind = "tag";
      } else if (combinators === 0 && (classes > 0 || attributes > 0 || pseudos > 0)) {
        kind = "direct";
      } else if (combinators > 0) {
        kind = "nested";
      }

      if (score > best.score) {
        best = { kind: kind, score: score };
      }
    });

    return best;
  }

  function collectMatchedCssRulesFromRuleList(rules, element, matches, seen, source, media) {
    Array.prototype.forEach.call(rules, function (rule) {
      if (rule instanceof CSSStyleRule) {
        try {
          if (!element.matches(rule.selectorText)) {
            return;
          }
        } catch (error) {
          return;
        }

        var key = source + "|" + (media || "") + "|" + rule.selectorText;
        if (seen[key]) {
          return;
        }

        seen[key] = true;
        var matchMeta = describeRuleMatch(rule.selectorText, element);
        matches.push({
          selector: rule.selectorText,
          source: source,
          media: media,
          declarations: Array.prototype.slice.call(rule.style).length,
          kind: matchMeta.kind,
          score: matchMeta.score,
        });
        return;
      }

      if (rule instanceof CSSMediaRule) {
        collectMatchedCssRulesFromRuleList(rule.cssRules, element, matches, seen, source, rule.conditionText);
        return;
      }

      if (rule instanceof CSSSupportsRule) {
        var nextMedia = media ? media + " | supports " + rule.conditionText : "supports " + rule.conditionText;
        collectMatchedCssRulesFromRuleList(rule.cssRules, element, matches, seen, source, nextMedia);
      }
    });
  }

  function collectMatchedCssRules(element) {
    var matches = [];
    var seen = {};
    var inlineStyle = element.getAttribute("style");

    if (inlineStyle && inlineStyle.trim()) {
      var declarationCount = inlineStyle
        .split(";")
        .map(function (entry) {
          return entry.trim();
        })
        .filter(Boolean).length;

      matches.push({
        selector: 'style=""',
        source: "inline",
        media: null,
        declarations: declarationCount,
        kind: "inline",
        score: 1000,
      });
    }

    Array.prototype.forEach.call(document.styleSheets, function (sheet) {
      var rules;
      try {
        rules = sheet.cssRules;
      } catch (error) {
        return;
      }

      if (!rules) {
        return;
      }

      collectMatchedCssRulesFromRuleList(
        rules,
        element,
        matches,
        seen,
        stylesheetSourceLabel(sheet.href),
        null
      );
    });

    return matches.sort(function (left, right) {
      return right.score - left.score || right.declarations - left.declarations;
    });
  }

  function extractVariableNames(value) {
    var matches = value.match(/var\(\s*(--[A-Za-z0-9_-]+)/g) || [];
    return matches.map(function (match) {
      return match.replace(/^var\(\s*/, "");
    });
  }

  function collectVariableNamesFromValue(value, variableNames) {
    extractVariableNames(value).forEach(function (name) {
      variableNames[name] = true;
    });
  }

  function collectVariableNamesFromRuleList(rules, element, variableNames) {
    Array.prototype.forEach.call(rules, function (rule) {
      if (rule instanceof CSSStyleRule) {
        try {
          if (!element.matches(rule.selectorText)) {
            return;
          }
        } catch (error) {
          return;
        }

        Array.prototype.forEach.call(rule.style, function (propertyName) {
          collectVariableNamesFromValue(rule.style.getPropertyValue(propertyName), variableNames);
        });
        return;
      }

      if (rule instanceof CSSMediaRule || rule instanceof CSSSupportsRule) {
        collectVariableNamesFromRuleList(rule.cssRules, element, variableNames);
      }
    });
  }

  function collectRelevantCssVariables(element) {
    var variableNames = {};
    var computed = window.getComputedStyle(element);

    collectVariableNamesFromValue(element.getAttribute("style") || "", variableNames);

    Array.prototype.forEach.call(document.styleSheets, function (sheet) {
      var rules;
      try {
        rules = sheet.cssRules;
      } catch (error) {
        return;
      }

      if (!rules) {
        return;
      }

      collectVariableNamesFromRuleList(rules, element, variableNames);
    });

    return Object.keys(variableNames)
      .map(function (name) {
        return {
          name: name,
          value:
            computed.getPropertyValue(name).trim() ||
            window.getComputedStyle(document.documentElement).getPropertyValue(name).trim(),
        };
      })
      .filter(function (variable) {
        return variable.value.length > 0;
      })
      .sort(function (left, right) {
        return left.name.localeCompare(right.name);
      });
  }

  function createSelectionInfo(element) {
    var computed = window.getComputedStyle(element);
    var rect = element.getBoundingClientRect();
    var tag = element.tagName.toLowerCase();
    var id = element.id || "";
    var classes = Array.prototype.filter.call(element.classList, function (className) {
      return className !== SELECTED_CLASS &&
        className !== TEMPLATE_SELECTED_CLASS &&
        className !== EMPTY_EDITABLE_CLASS &&
        className !== EMPTY_TERA_SLOT_CLASS;
    });
    var parentElement =
      element.parentElement && element.parentElement.tagName.toLowerCase() !== "html"
        ? element.parentElement
        : null;
    var childNodes = Array.prototype.slice
      .call(element.children)
      .filter(function (child) {
        return child instanceof Element && !isEmptyTeraSlot(child);
      })
      .slice(0, 24)
      .map(createDomNodeLink);
    var hasChildElements = Array.prototype.some.call(element.children, function (child) {
      return child instanceof Element && !isEmptyTeraSlot(child);
    });

    return {
      selector: formatElementSelector(tag, id, classes),
      cssSelector: createCssSelector(tag, id, classes),
      domPath: createDomPathSelector(element),
      tag: tag,
      id: id,
      href: element.getAttribute("href") || "",
      title: element.getAttribute("title") || "",
      alt: element.getAttribute("alt") || "",
      classes: classes,
      text: summarizeElementText(element.textContent),
      rawText: element.textContent || "",
      hasChildElements: hasChildElements,
      rect: {
        width: Math.round(rect.width) + "px",
        height: Math.round(rect.height) + "px",
        top: Math.round(rect.top) + "px",
        left: Math.round(rect.left) + "px",
      },
      styles: [
        { label: "color", value: computed.color },
        { label: "background", value: computed.backgroundColor },
        { label: "font-size", value: computed.fontSize },
        { label: "line-height", value: computed.lineHeight },
        { label: "text-align", value: computed.textAlign },
        { label: "font-weight", value: computed.fontWeight },
        { label: "display", value: computed.display },
        { label: "flex-direction", value: computed.flexDirection },
        { label: "justify-content", value: computed.justifyContent },
        { label: "align-items", value: computed.alignItems },
        { label: "gap", value: computed.gap },
        { label: "margin", value: formatBox(computed, "margin") },
        { label: "padding", value: formatBox(computed, "padding") },
        { label: "border-radius", value: computed.borderRadius },
      ],
      variables: collectRelevantCssVariables(element),
      matchedRules: collectMatchedCssRules(element),
      imageSrc: tag === "img" ? element.getAttribute("src") : null,
      zolaImage: tag === "img" ? decodeZolaImagePresentation(element) : null,
      attributes: collectElementAttributes(element),
	      parentNode: parentElement ? createDomNodeLink(parentElement) : null,
	      childNodes: childNodes,
	      sourceLocation: null,
	      sourceId: element.getAttribute(SOURCE_ID_ATTR) || null,
      templateSourceId: inheritedTemplateSourceId(element),
      sessionId: element.getAttribute(SESSION_ID_ATTR) || null,
      blockContext: blockContextForElement(element),
    };
  }

  function blockContextForElement(element) {
    var root = element.closest("[data-pana-block],[data-pana-component]");
    if (!root) return null;
    var canonical = root.getAttribute("data-pana-block");
    var legacy = root.getAttribute("data-pana-component");
    var providerId = (canonical || legacy || "").trim();
    if (!providerId) return null;
    return {
      providerId: providerId,
      markerKind: canonical ? "canonical" : "legacy",
      rootSelector: createDomPathSelector(root),
      rootTag: root.tagName.toLowerCase(),
      rootSourceId: root.getAttribute(SOURCE_ID_ATTR) || null,
      rootTemplateSourceId: inheritedTemplateSourceId(root),
      rootSessionId: root.getAttribute(SESSION_ID_ATTR) || null,
    };
  }

  function decodeZolaImagePresentation(element) {
    var payload = element.getAttribute("data-pana-zola-image");
    if (!payload) return null;
    try {
      var standard = payload.replace(/-/g, "+").replace(/_/g, "/");
      while (standard.length % 4 !== 0) standard += "=";
      var binary = window.atob(standard);
      var bytes = new Uint8Array(binary.length);
      for (var index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
      }
      var candidate = JSON.parse(new TextDecoder().decode(bytes));
      if (!candidate || typeof candidate.sourceUrl !== "string" || typeof candidate.sourcePath !== "string") {
        return null;
      }
      return candidate;
    } catch (error) {
      return null;
    }
  }
