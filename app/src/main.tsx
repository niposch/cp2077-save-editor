import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import "./index.css"
import App from "./App.tsx"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { SaveProvider } from "@/state/save-context"
import { initCore } from "@/lib/core"

initCore()

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <TooltipProvider delayDuration={250}>
      <SaveProvider>
        <App />
        <Toaster theme="dark" position="bottom-right" />
      </SaveProvider>
    </TooltipProvider>
  </StrictMode>,
)
