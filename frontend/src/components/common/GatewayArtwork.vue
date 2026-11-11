<template>
  <div class="gateway-artwork" aria-hidden="true">
    <svg viewBox="0 0 1400 900" fill="none" preserveAspectRatio="xMidYMid slice">
      <defs>
        <linearGradient
          id="spectrum-field"
          x1="198"
          y1="682"
          x2="1203"
          y2="110"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#A8EAED" />
          <stop offset=".35" stop-color="#B4BDFF" />
          <stop offset=".68" stop-color="#B9A0FF" />
          <stop offset="1" stop-color="#FFB2BA" />
        </linearGradient>
        <linearGradient
          id="spectrum-warm"
          x1="921"
          y1="251"
          x2="1157"
          y2="671"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#FFD58D" />
          <stop offset=".48" stop-color="#FFAFAD" />
          <stop offset="1" stop-color="#DF9EEA" />
        </linearGradient>
        <linearGradient
          id="spectrum-cool"
          x1="392"
          y1="260"
          x2="762"
          y2="690"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#91ECED" />
          <stop offset=".5" stop-color="#A1CAFF" />
          <stop offset="1" stop-color="#B9A4FF" />
        </linearGradient>
        <filter id="spectrum-soft"><feGaussianBlur stdDeviation="36" /></filter>
      </defs>
      <g class="spectrum-drift" filter="url(#spectrum-soft)">
        <path
          d="M-77 504C230 506 193 57 530 129c337 72 283 432 553 372 219-50 294-349 471-323v705H-77V504Z"
          fill="url(#spectrum-field)"
        />
        <path
          d="M679-80c139 160 121 416 267 485 129 61 192-10 401 140l132 334-594 141C1120 657 598 487 530 80L679-80Z"
          fill="url(#spectrum-warm)"
        />
        <path
          d="M-107 741c139-350 431-540 580-397 155 148 198 225 480 236l-36 284H-107V741Z"
          fill="url(#spectrum-cool)"
        />
        <path
          d="M1041-58c-92 148-68 254 51 375 120 121 130 235 55 418l292-31V-58h-398Z"
          fill="#FFD8A0"
          opacity=".65"
        />
      </g>
    </svg>
  </div>
</template>
<style scoped>
.gateway-artwork {
  width: 100%;
  height: 100%;
  overflow: hidden;
  pointer-events: none;
}
.gateway-artwork svg {
  display: block;
  width: 100%;
  height: 100%;
}
.spectrum-drift {
  transform-origin: center;
  animation: spectrum-shift 24s ease-in-out infinite alternate;
}
@keyframes spectrum-shift {
  from {
    transform: translate(-2%, -2%) scale(1.05);
  }
  to {
    transform: translate(2%, 3%) scale(1.1);
  }
}
@media (prefers-reduced-motion: reduce) {
  .spectrum-drift {
    animation: none;
  }
}
</style>
