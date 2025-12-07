import { useEffect, useRef } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import hljs from "highlight.js";
import "highlight.js/styles/github.min.css";

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    // Configure marked for GitHub Flavored Markdown
    marked.setOptions({
      breaks: true,
      gfm: true,
    });

    // Create a custom renderer for code blocks with syntax highlighting
    const renderer = new marked.Renderer();
    renderer.code = function({ text, lang }) {
      const code = text;
      const language = lang || "";
      if (language && hljs.getLanguage(language)) {
        try {
          const highlighted = hljs.highlight(code, { language }).value;
          return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
        } catch (err) {
          // Fallback if highlighting fails
        }
      }
      const highlighted = hljs.highlightAuto(code).value;
      return `<pre><code class="hljs">${highlighted}</code></pre>`;
    };

    // Convert markdown to HTML (marked.parse can return a string or Promise)
    const parseResult = marked.parse(content, { renderer });
    const processHtml = (html: string) => {
      if (!containerRef.current) return;
      
      // Sanitize HTML
      const sanitized = DOMPurify.sanitize(html, {
        ALLOWED_TAGS: [
          "p", "br", "strong", "em", "u", "s", "h1", "h2", "h3", "h4", "h5", "h6",
          "ul", "ol", "li", "blockquote", "pre", "code", "a", "img", "table",
          "thead", "tbody", "tr", "th", "td", "hr", "div", "span"
        ],
        ALLOWED_ATTR: ["href", "src", "alt", "title", "class", "id"],
        ALLOW_DATA_ATTR: false,
      });

      containerRef.current.innerHTML = sanitized;

      // Add copy buttons to code blocks
      const codeBlocks = containerRef.current.querySelectorAll("pre code");
      codeBlocks.forEach((block) => {
        const pre = block.parentElement;
        if (pre && !pre.querySelector(".copy-button")) {
          const copyButton = document.createElement("button");
          copyButton.className = "copy-button";
          copyButton.textContent = "Copier";
          copyButton.onclick = () => {
            navigator.clipboard.writeText(block.textContent || "");
            copyButton.textContent = "Copié!";
            setTimeout(() => {
              copyButton.textContent = "Copier";
            }, 2000);
          };
          pre.style.position = "relative";
          pre.appendChild(copyButton);
        }
      });
    };

    if (typeof parseResult === "string") {
      processHtml(parseResult);
    } else {
      parseResult.then(processHtml);
    }
  }, [content]);

  return (
    <div
      ref={containerRef}
      className="markdown-content"
      style={{
        lineHeight: "1.6",
        wordWrap: "break-word",
      }}
    />
  );
}

