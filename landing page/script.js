const navbar = document.getElementById('navbar');
const progressBar = document.querySelector('.scroll-progress');
const reveals = document.querySelectorAll('.reveal');
const tiltCard = document.querySelector('[data-tilt]');
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

function updateChrome() {
  const scrollable = document.documentElement.scrollHeight - window.innerHeight;
  const progress = scrollable > 0 ? window.scrollY / scrollable : 0;

  navbar?.classList.toggle('scrolled', window.scrollY > 36);
  if (progressBar) {
    progressBar.style.transform = `scaleX(${Math.min(Math.max(progress, 0), 1)})`;
  }
}

const revealObserver = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) {
        return;
      }

      entry.target.classList.add('visible');
      revealObserver.unobserve(entry.target);
    });
  },
  {
    threshold: 0.12,
    rootMargin: '0px 0px -44px 0px',
  },
);

reveals.forEach((element, index) => {
  element.style.transitionDelay = `${Math.min(index % 6, 4) * 55}ms`;
  revealObserver.observe(element);
});

document.querySelectorAll('.features-grid, .usecases-grid').forEach((grid) => {
  grid.querySelectorAll('.feature-card, .usecase-card').forEach((card, index) => {
    card.style.transitionDelay = `${index * 55}ms`;
  });
});

window.addEventListener('scroll', updateChrome, { passive: true });
window.addEventListener('resize', updateChrome);
updateChrome();

window.addEventListener('pointermove', (event) => {
  if (tiltCard && window.innerWidth < 1080) {
    tiltCard.style.transform = '';
  }

  if (!tiltCard || reducedMotion.matches || window.innerWidth < 1080) {
    return;
  }

  const rect = tiltCard.getBoundingClientRect();
  const x = (event.clientX - rect.left) / rect.width - 0.5;
  const y = (event.clientY - rect.top) / rect.height - 0.5;

  if (Math.abs(x) > 1 || Math.abs(y) > 1) {
    tiltCard.style.transform = 'translateY(-44%)';
    return;
  }

  tiltCard.style.transform = `translateY(-44%) rotateX(${-y * 7}deg) rotateY(${x * 7}deg)`;
});

window.addEventListener('pointerleave', () => {
  if (tiltCard) {
    tiltCard.style.transform = '';
  }
});
