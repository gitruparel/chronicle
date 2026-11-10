import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ErrorBoundary } from './components/ErrorBoundary';
import './index.css';

// Disable standard web browser accelerators (F5, Ctrl+R, Ctrl+F5, Ctrl+Shift+R)
// in the root capture phase so Chronicle behaves strictly as a native desktop app
// and never drops into a blank WebView2 canvas.
window.addEventListener(
  'keydown',
  (e) => {
    if (
      e.key === 'F5' ||
      ((e.ctrlKey || e.metaKey) && (e.key === 'r' || e.key === 'R'))
    ) {
      e.preventDefault();
      e.stopPropagation();
    }
  },
  { capture: true }
);

// Suppress the default web browser context menu (Inspect, Back, Reload)
// while allowing native text selection/context menu in inputs and textareas
window.addEventListener(
  'contextmenu',
  (e) => {
    const target = e.target as HTMLElement | null;
    if (
      target &&
      (target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable)
    ) {
      return;
    }
    e.preventDefault();
  },
  { capture: true }
);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);
