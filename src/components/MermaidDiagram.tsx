import { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";

let mermaidInitialized = false;

function ensureMermaidInit() {
  if (!mermaidInitialized) {
    mermaid.initialize({
      startOnLoad: false,
      theme: "dark",
      themeVariables: {
        primaryColor: "#4a9eff",
        primaryTextColor: "#e0e0e0",
        primaryBorderColor: "#5a5a5a",
        lineColor: "#888",
        secondaryColor: "#2d2d2d",
        tertiaryColor: "#1a1a1a",
        fontFamily: "inherit",
        fontSize: "14px",
      },
      flowchart: {
        curve: "basis",
        padding: 12,
        htmlLabels: true,
      },
    });
    mermaidInitialized = true;
  }
}

let diagramCounter = 0;

interface MermaidDiagramProps {
  code: string;
}

export function MermaidDiagram({ code }: MermaidDiagramProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [idRef] = useState(() => `mermaid-${Date.now()}-${diagramCounter++}`);

  useEffect(() => {
    if (!containerRef.current) return;

    ensureMermaidInit();

    let cancelled = false;

    (async () => {
      try {
        const { svg } = await mermaid.render(idRef, code);
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = svg;
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "図の描画に失敗しました");
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [code, idRef]);

  if (error) {
    return (
      <div style={styles.errorContainer}>
        <div style={styles.errorLabel}>図の描画エラー</div>
        <pre style={styles.fallbackCode}>{code}</pre>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div ref={containerRef} style={styles.diagram} />
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    margin: "8px 0",
    padding: "12px",
    background: "rgba(0, 0, 0, 0.2)",
    borderRadius: "8px",
    overflow: "auto",
  },
  diagram: {
    display: "flex",
    justifyContent: "center",
  },
  errorContainer: {
    margin: "8px 0",
    padding: "8px",
    background: "rgba(255, 100, 100, 0.1)",
    borderRadius: "8px",
    border: "1px solid rgba(255, 100, 100, 0.2)",
  },
  errorLabel: {
    fontSize: "11px",
    color: "var(--text-muted)",
    marginBottom: "4px",
  },
  fallbackCode: {
    fontSize: "12px",
    fontFamily: "monospace",
    color: "var(--text-secondary)",
    whiteSpace: "pre-wrap",
    margin: 0,
  },
};
