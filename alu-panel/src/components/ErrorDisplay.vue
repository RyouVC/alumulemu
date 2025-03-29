<template>
  <div class="flex flex-col items-center justify-center px-4 text-center">
    <!-- Large error icon -->
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class="h-32 w-32 text-error"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
      />
    </svg>

    <!-- Error message at bottom -->
    <div class="mt-8 text-error text-xl font-bold">{{ message }}</div>
  </div>
</template>

<script>
export default {
  name: "ErrorDisplay",
  props: {
    message: {
      type: String,
      required: true,
      default: "An error occurred",
    },
  },
};
</script>
