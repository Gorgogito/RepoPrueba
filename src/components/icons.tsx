import type { SVGProps } from "react";

function base(props: SVGProps<SVGSVGElement>) {
  return {
    viewBox: "0 0 16 16",
    width: 14,
    height: 14,
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.6,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    ...props,
  };
}

export function IconMerge(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <circle cx="4" cy="12" r="1.6" />
      <circle cx="12" cy="4" r="1.6" />
      <circle cx="12" cy="12" r="1.6" />
      <path d="M12 5.6V10.4" />
      <path d="M4 10.4C4 6.5 7.5 4.5 10.4 4.2" />
    </svg>
  );
}

export function IconRebase(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M3 13h10" />
      <path d="M8 11V3" />
      <path d="M5 6l3-3 3 3" />
    </svg>
  );
}

export function IconClose(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M4 4l8 8" />
      <path d="M12 4l-8 8" />
    </svg>
  );
}

export function IconCherryPick(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <circle cx="5" cy="12" r="2" />
      <circle cx="10.5" cy="12.8" r="2" />
      <path d="M5 10.2C5 6.5 6.8 4.2 9 3" />
      <path d="M10.5 11C10.5 8 9.8 5 9 3" />
      <path d="M9 3c1-0.8 2-0.8 2.5 0.2" />
    </svg>
  );
}

export function IconRevert(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M4.5 7H10a3 3 0 0 1 0 6H7" />
      <path d="M6.5 4L3 7l3.5 3" />
    </svg>
  );
}

export function IconDiff(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <rect x="2.2" y="3" width="4.8" height="10" rx="1" />
      <rect x="9" y="3" width="4.8" height="10" rx="1" />
    </svg>
  );
}

export function IconChevronDown(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M4 6l4 4 4-4" />
    </svg>
  );
}

export function IconDot(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)} fill="currentColor" stroke="none">
      <circle cx="8" cy="8" r="4" />
    </svg>
  );
}

export function IconWarning(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M8 2L15 14H1L8 2Z" strokeLinejoin="round" />
      <path d="M8 6.5V10" />
      <path d="M8 12.2v0.1" strokeLinecap="round" />
    </svg>
  );
}

export function IconStashPop(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M8 2v7" />
      <path d="M5 6.5l3 3 3-3" />
      <path d="M3 13h10" />
    </svg>
  );
}

export function IconStashApply(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...base(props)}>
      <path d="M8 2V7" />
      <path d="M5.5 4.8l2.5 2.5 2.5-2.5" />
      <rect x="3" y="10" width="10" height="3" rx="1" />
    </svg>
  );
}
