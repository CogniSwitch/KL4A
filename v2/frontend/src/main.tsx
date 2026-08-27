import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router-dom'
import './index.css'
import App from './App.tsx'
import { notify_frontend_ready } from './lib/api'

// HashRouter, not BrowserRouter: the built frontend is loaded by Tauri from
// a local asset root with no server to rewrite arbitrary sub-paths back to
// index.html, so history-API routing would 404 on refresh/deep-link.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
)

// Startup-diagnostics checkpoint 3 -- see startup_log.rs. Fired once the render
// call above has synchronously kicked off (not necessarily painted yet, but far
// enough that module evaluation and the initial render pass succeeded).
notify_frontend_ready()
