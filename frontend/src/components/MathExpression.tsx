import katex from 'katex';

interface MathExpressionProps {
  expression: string;
  display: boolean;
}

function renderMath(expression: string, display: boolean) {
  try {
    return katex.renderToString(expression, {
      displayMode: display,
      output: 'htmlAndMathml',
      throwOnError: true,
      strict: 'ignore',
      trust: false,
      maxExpand: 1000,
    });
  } catch {
    return null;
  }
}

export function MathExpression({ expression, display }: MathExpressionProps) {
  const html = renderMath(expression, display);
  if (!html) {
    return (
      <span className={display ? 'aegis-math-fallback aegis-math-display' : 'aegis-math-fallback'}>
        {expression}
      </span>
    );
  }

  return (
    <span
      className={display ? 'aegis-math aegis-math-display' : 'aegis-math aegis-math-inline'}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
