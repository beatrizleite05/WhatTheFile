import type { Transition, Variants } from 'framer-motion';

export const springSnappy: Transition = {
  type: 'spring',
  stiffness: 500,
  damping: 30,
  mass: 0.5,
};

export const springGentle: Transition = {
  type: 'spring',
  stiffness: 300,
  damping: 28,
  mass: 0.8,
};

export const fadeSlide: Variants = {
  initial: { opacity: 0, y: -6 },
  animate: { opacity: 1, y: 0, transition: springSnappy },
  exit: { opacity: 0, y: -6, transition: { duration: 0.12 } },
};

export const fadeIn: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.15 } },
  exit: { opacity: 0, transition: { duration: 0.1 } },
};

export const slideRight: Variants = {
  initial: { opacity: 0, x: 16 },
  animate: { opacity: 1, x: 0, transition: springGentle },
  exit: { opacity: 0, x: 16, transition: { duration: 0.12 } },
};
