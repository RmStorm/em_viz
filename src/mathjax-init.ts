export async function initMathJax() {
  (window as any).MathJax = {
    tex: { inlineMath: [['\\(','\\)']], displayMath: [['$$','$$']] },
    svg: { fontCache: 'global' },
    startup: { typeset: false } // we’ll typeset manually
  };

  // dynamic import so Vite bundles it correctly
  await import('mathjax/es5/tex-svg.js');

  // wait for MathJax startup, then typeset the page
  await (window as any).MathJax.startup.promise;
  await (window as any).MathJax.typesetPromise();
}
