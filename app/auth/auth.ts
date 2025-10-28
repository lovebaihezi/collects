import { StackClientApp } from "@stackframe/js";
import { timeout } from "signal-timers";

const stackServerApp = new StackClientApp({
  projectId: "aa1dbdfb-0086-4166-91a0-f1fa1b29230d",
  publishableClientKey: "pck_sy6cs82c6dr476mdpmh8d59y9et4r293asxq5nfnwva58",
  tokenStore: "cookie",
});

function showToast(
  message: string,
  type: "info" | "success" | "warning" | "error" = "info",
  signal: AbortSignal,
) {
  const toastContainer = document.getElementById("toast-container");
  if (!toastContainer) return;

  const typeStyles = {
    info: "bg-blue-500",
    success: "bg-green-500",
    warning: "bg-yellow-500",
    error: "bg-red-500",
  };

  const toastId = `toast-${Date.now()}`;
  const toast = document.createElement("div");
  toast.id = toastId;
  toast.className = `${
    typeStyles[type]
  } text-white px-6 py-4 rounded-lg shadow-lg min-w-[300px] transform transition-all duration-300 ease-in-out`;

  toast.innerHTML = `
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <svg class="w-5 h-5 shrink-0" fill="currentColor" viewBox="0 0 20 20">
          ${
    type === "success"
      ? `
            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
          `
      : type === "error"
      ? `
            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd"/>
          `
      : `
            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"/>
          `
  }
        </svg>
        <span class="font-medium">${message}</span>
      </div>
      <button class="text-white hover:text-gray-200 transition-colors" onclick="this.closest('[id^=toast-]').remove()">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
        </svg>
      </button>
    </div>
    <div class="mt-2 h-1 bg-white bg-opacity-30 rounded-full overflow-hidden">
      <div class="progress-bar h-full bg-white transition-all ease-linear" style="width: 100%"></div>
    </div>
  `;

  toastContainer.appendChild(toast);

  // Animate in
  setTimeout(() => {
    toast.style.opacity = "1";
  }, 10);

  // Progress animation
  const progressBar = toast.querySelector(".progress-bar") as HTMLElement;
  let progress = 100;
  const interval = 30; // 3000ms / 100 steps = 30ms per step

  const updateProgress = () => {
    if (signal.aborted) return;

    progress -= 1;
    if (progressBar) {
      progressBar.style.width = `${progress}%`;
    }

    if (progress <= 0) {
      removeToast(toastId, signal);
    } else {
      timeout(updateProgress, interval, { signal });
    }
  };

  timeout(updateProgress, interval, { signal });

  signal.addEventListener("abort", () => {
    removeToast(toastId, signal);
  });

  return toastId;
}

function removeToast(toastId: string, signal: AbortSignal) {
  const toast = document.getElementById(toastId);
  if (toast) {
    toast.style.opacity = "0";
    toast.style.transform = "translateX(100%)";
    timeout(
      () => {
        toast.remove();
      },
      300,
      { signal },
    );
  }
}

function timeoutPromise<T>(
  callback: () => T,
  ms: number,
  signal: AbortSignal,
): Promise<T> {
  return new Promise<T>((resolve) => {
    timeout(
      () => {
        resolve(callback());
      },
      ms,
      { signal },
    );
  });
}

async function handleGoogleLogin(signal: AbortSignal) {
}

async function handleGithubLogin(signal: AbortSignal) {
}

async function handleEmailPasswordLogin(
  email: string,
  password: string,
  signal: AbortSignal,
) {
  try {
    const authResult = await stackServerApp.signInWithCredential({
      email,
      password,
      noRedirect: true,
    });
    signal.throwIfAborted();

    switch (authResult.status) {
      case "ok": {
        await fetch("/auth-callback", {
          method: "POST",
          credentials: "include",
        });
        break;
      }
      case "error": {
        showToast(`Login failed: ${authResult.error}`, "error", signal);
        break;
      }
    }
  } catch (error) {
    console.error("Login error:", error);
    if (!signal.aborted) {
      showToast("An unexpected error occurred", "error", signal);
    }
  }
}

function main(signal: AbortSignal) {
  const emailInput = document.querySelector(
    "#login-email-input",
  )! as HTMLInputElement;
  const passwordInput = document.querySelector(
    "#login-password-input",
  )! as HTMLInputElement;
  const loginButton = document.querySelector(
    "#login-button",
  )! as HTMLButtonElement;
  const googleButton = document.querySelector(
    "#google-login-button",
  )! as HTMLButtonElement;
  const githubButton = document.querySelector(
    "#github-login-button",
  )! as HTMLButtonElement;
  const loginForm = loginButton.closest("form")!;

  // Handle email/password login
  loginForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const email = emailInput.value.trim();
    const password = passwordInput.value;

    if (!email || !password) {
      showToast("Please enter both email and password", "warning", signal);
      return;
    }

    loginButton.disabled = true;
    loginButton.textContent = "Signing in...";

    await handleEmailPasswordLogin(email, password, signal);

    loginButton.disabled = false;
    loginButton.textContent = "Sign In";
  }, { signal });

  // Handle Google OAuth login
  googleButton.addEventListener("click", async () => {
    googleButton.disabled = true;
    await handleGoogleLogin(signal);
    googleButton.disabled = false;
  }, { signal });

  // Handle GitHub OAuth login
  githubButton.addEventListener("click", async () => {
    githubButton.disabled = true;
    await handleGithubLogin(signal);
    githubButton.disabled = false;
  }, { signal });
}

const abortController = new AbortController();

main(abortController.signal);

self.addEventListener("beforeunload", () => {
  abortController.abort();
});
