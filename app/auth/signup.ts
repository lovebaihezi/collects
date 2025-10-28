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

async function handleGoogleSignup(signal: AbortSignal) {
  try {
    showToast("Redirecting to Google...", "info", signal);
    // Add Google OAuth signup logic here
    // For now, we'll show a placeholder message
    await timeout(() => {}, 1000, { signal });
    showToast("Google signup is not yet configured", "warning", signal);
  } catch (error) {
    console.error("Google signup error:", error);
    showToast("Failed to initiate Google signup", "error", signal);
  }
}

async function handleGithubSignup(signal: AbortSignal) {
  try {
    showToast("Redirecting to GitHub...", "info", signal);
    // Add GitHub OAuth signup logic here
    // For now, we'll show a placeholder message
    await timeout(() => {}, 1000, { signal });
    showToast("GitHub signup is not yet configured", "warning", signal);
  } catch (error) {
    console.error("GitHub signup error:", error);
    showToast("Failed to initiate GitHub signup", "error", signal);
  }
}

function validatePassword(
  password: string,
): { valid: boolean; message?: string } {
  if (password.length < 8) {
    return {
      valid: false,
      message: "Password must be at least 8 characters long",
    };
  }
  if (!/[A-Z]/.test(password)) {
    return {
      valid: false,
      message: "Password must contain at least one uppercase letter",
    };
  }
  if (!/[a-z]/.test(password)) {
    return {
      valid: false,
      message: "Password must contain at least one lowercase letter",
    };
  }
  if (!/[0-9]/.test(password)) {
    return {
      valid: false,
      message: "Password must contain at least one number",
    };
  }
  return { valid: true };
}

function validateEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

async function handleEmailPasswordSignup(
  email: string,
  password: string,
  confirmPassword: string,
  signal: AbortSignal,
) {
  try {
    // Validate email
    if (!validateEmail(email)) {
      showToast("Please enter a valid email address", "error", signal);
      return;
    }

    // Validate password
    const passwordValidation = validatePassword(password);
    if (!passwordValidation.valid) {
      showToast(passwordValidation.message!, "error", signal);
      return;
    }

    // Check password confirmation
    if (password !== confirmPassword) {
      showToast("Passwords do not match", "error", signal);
      return;
    }

    const authResult = await stackServerApp.signUpWithCredential({
      email,
      password,
      noRedirect: true,
    });
    signal.throwIfAborted();

    switch (authResult.status) {
      case "ok": {
        showToast(
          "Account created successfully! Redirecting...",
          "success",
          signal,
        );
        await fetch("/auth-callback", {
          method: "POST",
          credentials: "include",
        });
        // Redirect to main app or dashboard
        timeout(
          () => {
            window.location.href = "/";
          },
          1500,
          { signal },
        );
        break;
      }
      case "error": {
        showToast(`Signup failed: ${authResult.error}`, "error", signal);
        break;
      }
    }
  } catch (error) {
    console.error("Signup error:", error);
    if (!signal.aborted) {
      showToast("An unexpected error occurred", "error", signal);
    }
  }
}

function main(signal: AbortSignal) {
  const emailInput = document.querySelector(
    "#signup-email-input",
  )! as HTMLInputElement;
  const passwordInput = document.querySelector(
    "#signup-password-input",
  )! as HTMLInputElement;
  const confirmPasswordInput = document.querySelector(
    "#signup-confirm-password-input",
  )! as HTMLInputElement;
  const signupButton = document.querySelector(
    "#signup-button",
  )! as HTMLButtonElement;
  const googleButton = document.querySelector(
    "#google-signup-button",
  )! as HTMLButtonElement;
  const githubButton = document.querySelector(
    "#github-signup-button",
  )! as HTMLButtonElement;
  const signupForm = signupButton.closest("form")!;

  // Handle email/password signup
  signupForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const email = emailInput.value.trim();
    const password = passwordInput.value;
    const confirmPassword = confirmPasswordInput.value;

    if (!email || !password || !confirmPassword) {
      showToast("Please fill in all fields", "warning", signal);
      return;
    }

    signupButton.disabled = true;
    signupButton.textContent = "Creating account...";

    await handleEmailPasswordSignup(email, password, confirmPassword, signal);

    signupButton.disabled = false;
    signupButton.textContent = "Create Account";
  }, { signal });

  // Handle Google OAuth signup
  googleButton.addEventListener("click", async () => {
    googleButton.disabled = true;
    await handleGoogleSignup(signal);
    googleButton.disabled = false;
  }, { signal });

  // Handle GitHub OAuth signup
  githubButton.addEventListener("click", async () => {
    githubButton.disabled = true;
    await handleGithubSignup(signal);
    githubButton.disabled = false;
  }, { signal });

  // Real-time password validation feedback
  passwordInput.addEventListener("input", () => {
    const password = passwordInput.value;
    if (password.length > 0) {
      const validation = validatePassword(password);
      if (!validation.valid) {
        passwordInput.classList.add("border-red-300");
        passwordInput.classList.remove("border-gray-300");
      } else {
        passwordInput.classList.remove("border-red-300");
        passwordInput.classList.add("border-green-300");
      }
    } else {
      passwordInput.classList.remove("border-red-300", "border-green-300");
      passwordInput.classList.add("border-gray-300");
    }
  }, { signal });

  // Real-time password confirmation feedback
  confirmPasswordInput.addEventListener("input", () => {
    const password = passwordInput.value;
    const confirmPassword = confirmPasswordInput.value;
    if (confirmPassword.length > 0) {
      if (password !== confirmPassword) {
        confirmPasswordInput.classList.add("border-red-300");
        confirmPasswordInput.classList.remove("border-gray-300");
      } else {
        confirmPasswordInput.classList.remove("border-red-300");
        confirmPasswordInput.classList.add("border-green-300");
      }
    } else {
      confirmPasswordInput.classList.remove(
        "border-red-300",
        "border-green-300",
      );
      confirmPasswordInput.classList.add("border-gray-300");
    }
  }, { signal });
}

const abortController = new AbortController();

main(abortController.signal);

window.addEventListener("beforeunload", () => {
  abortController.abort();
});
