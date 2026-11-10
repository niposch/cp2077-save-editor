import path from 'node:path'
import fs from 'node:fs'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig, type Plugin } from 'vite'

/**
 * Dev-only: serves the project's *copied* saves (../saves) so the UI can be exercised
 * without a file picker. Never part of a production build.
 *   GET /__dev/saves            -> ["ManualSave-17", ...]
 *   GET /__dev/saves/<dir>/<f>  -> file bytes
 */
function devSaves(): Plugin {
  const root = path.resolve(__dirname, '../saves')
  return {
    name: 'dev-saves',
    apply: 'serve',
    configureServer(server) {
      // POST /__dev/out/<file>: lets automated tests capture what the browser would download.
      // Writes only into ../work/web-out (git-ignored scratch).
      server.middlewares.use('/__dev/out', (req, res) => {
        if (req.method !== 'POST') {
          res.statusCode = 405
          return res.end()
        }
        const outDir = path.resolve(__dirname, '../work/web-out')
        const name = path.basename(decodeURIComponent((req.url ?? '').replace(/^\/+/, '')))
        if (!name) {
          res.statusCode = 400
          return res.end()
        }
        fs.mkdirSync(outDir, { recursive: true })
        const chunks: Buffer[] = []
        req.on('data', (c) => chunks.push(c))
        req.on('end', () => {
          fs.writeFileSync(path.join(outDir, name), Buffer.concat(chunks))
          res.end('ok')
        })
      })
      server.middlewares.use('/__dev/saves', (req, res) => {
        const rel = decodeURIComponent((req.url ?? '/').split('?')[0]).replace(/^\/+/, '')
        const target = path.resolve(root, rel)
        if (!target.startsWith(root) || !fs.existsSync(target)) {
          res.statusCode = 404
          return res.end()
        }
        if (fs.statSync(target).isDirectory()) {
          const dirs = fs.readdirSync(target).filter((d) => fs.existsSync(path.join(target, d, 'sav.dat')))
          res.setHeader('content-type', 'application/json')
          return res.end(JSON.stringify(dirs))
        }
        res.end(fs.readFileSync(target))
      })
    },
  }
}

/**
 * Example saves (../examples) for the website's "Try an example save":
 * served at `examples/` by the dev server and copied into `dist/examples` by web builds.
 * Desktop builds (run by the Tauri CLI, which sets TAURI_ENV_PLATFORM) leave them out.
 */
function exampleSaves(): Plugin {
  const src = path.resolve(__dirname, '../examples')
  const isDesktop = !!process.env.TAURI_ENV_PLATFORM
  let outDir = ''
  return {
    name: 'example-saves',
    configResolved(c) {
      outDir = path.resolve(c.root, c.build.outDir)
    },
    configureServer(server) {
      server.middlewares.use('/examples', (req, res, next) => {
        const rel = decodeURIComponent((req.url ?? '/').split('?')[0]).replace(/^\/+/, '')
        const target = path.resolve(src, rel)
        if (!target.startsWith(src + path.sep) || !fs.existsSync(target) || fs.statSync(target).isDirectory()) return next()
        res.end(fs.readFileSync(target))
      })
    },
    closeBundle() {
      if (isDesktop || !outDir || !fs.existsSync(src)) return
      fs.cpSync(src, path.join(outDir, 'examples'), { recursive: true, filter: (p) => !p.endsWith('README.md') })
    },
  }
}

export default defineConfig({
  // relative asset paths so the site works under a GitHub Pages sub-path
  base: './',
  plugins: [react(), tailwindcss(), devSaves(), exampleSaves()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  // Tauri expects a fixed port and doesn't want the screen cleared
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2022' },
})
