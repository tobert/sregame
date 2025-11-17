## Headless Mode Implementation

**Goal:** Implement a reliable headless testing environment for the Bevy 0.17 application, primarily for automated runs and use with the Bevy Remote Protocol.

**Progress:**
1.  **Lifetime Control:** Added `--frames <N>` and `--seconds <N>` command-line arguments to allow the application to run for a fixed duration and then exit gracefully. This is fully implemented and working.
2.  **Headless Alias:** Created a `cargo headless` alias that uses the Cage Wayland compositor to provide a virtual display environment, allowing the Bevy application to run without a physical monitor.
3.  **Code Simplification:** Refactored `main.rs` to remove all complex headless-specific logic. The application now uses a standard `DefaultPlugins` setup, making it agnostic to whether it's running in a graphical or headless environment.

**Current Problem:**
- The application consistently hangs (times out) when launched via `cargo headless`.
- No application logs are produced, indicating the hang occurs very early in the Bevy initialization process, likely within the `winit` backend's connection to the Wayland compositor (`cage`).

**Next Steps:**
The direct implementation is currently blocked. The immediate next step is to conduct a thorough research task to understand the specific requirements and potential pitfalls of running a Bevy 0.17 application within Cage.

---

### 💎 Research Prompt for Gemini Agent

**Objective:** Create a comprehensive guide on running a Bevy 0.17 application headlessly on Arch Linux using the Cage Wayland compositor.

**Key Research Areas:**
1.  **Bevy + `winit` + Wayland Interaction:** Deep dive into how Bevy's `winit` backend initializes on Wayland. What specific Wayland protocols and environment variables (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, etc.) are absolutely required for it to succeed?
2.  **Cage Environment:** What environment does `cage` provide to its child processes? Does it fully implement all necessary protocols for a complex graphical application like Bevy? Are there any known limitations or required configurations?
3.  **Troubleshooting the Hang:** Investigate potential causes for a Bevy application to hang *before* the first `Update` loop when run inside `cage`. This should include checking for deadlocks in `winit`, missing GPU resources, or Wayland protocol mismatches.
4.  **Best Practices:** Provide a step-by-step, verifiable example of a simple Bevy 0.17 application successfully running and exiting within `cage`. Include the necessary code, `Cargo.toml` dependencies, and the exact `cage` command to run it.
5.  **Alternative Compositors:** Briefly evaluate if other headless Wayland compositors (e.g., `sway --unsupported-gpu`, `wlroots` headless backend) are better suited for this task and why.

**Deliverable:** A Markdown document summarizing the findings with actionable code examples and configuration steps.