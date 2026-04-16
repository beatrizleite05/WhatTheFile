import { useState } from 'react';
import { useSearch } from './hooks/useSearch';
import { useIndexing } from './hooks/useIndexing';
import { useSettings } from './hooks/useSettings';
import { useOllamaStatus } from './hooks/useOllamaStatus';
import { OnboardingFlow } from './components/OnboardingFlow';
import { FramelessOverlay } from './components/FramelessOverlay';

export default function App() {
  const settings = useSettings();
  const ollamaStatus = useOllamaStatus();
  const indexing = useIndexing();
  const search = useSearch();

  const [onboarded, setOnboarded] = useState(
    () => localStorage.getItem('wtf:onboarded') === 'true'
  );
  const [skippedOnboarding, setSkippedOnboarding] = useState(
    () => localStorage.getItem('wtf:skippedOnboarding') === 'true'
  );

  const showOnboarding = !onboarded;

  if (showOnboarding) {
    return (
      <OnboardingFlow
        onComplete={(skipped) => {
          localStorage.setItem('wtf:onboarded', 'true');
          if (skipped) {
            localStorage.setItem('wtf:skippedOnboarding', 'true');
            setSkippedOnboarding(true);
          } else {
            localStorage.removeItem('wtf:skippedOnboarding');
            setSkippedOnboarding(false);
          }
          setOnboarded(true);
        }}
        settings={settings}
      />
    );
  }

  return (
    <FramelessOverlay
      search={search}
      indexing={indexing}
      ollamaStatus={ollamaStatus}
      showSkipWarning={skippedOnboarding && settings.roots.length === 0}
    />
  );
}
