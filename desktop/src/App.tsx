import { useState, useEffect } from 'react';
import { useSearch } from './hooks/useSearch';
import { useIndexing, IndexingProvider } from './hooks/useIndexing';
import { useSettings } from './hooks/useSettings';
import { useOllamaStatus } from './hooks/useOllamaStatus';
import { OnboardingFlow } from './components/OnboardingFlow';
import { FramelessOverlay } from './components/FramelessOverlay';
import { SettingsView } from './components/SettingsView';

type View = 'main' | 'settings';

function AppInner() {
  const indexing = useIndexing();
  const settings = useSettings(indexing.startIndexing);
  const ollamaStatus = useOllamaStatus();
  const search = useSearch();

  const [view, setView] = useState<View>('main');
  const [onboarded, setOnboarded] = useState(
    () => localStorage.getItem('wtf:onboarded') === 'true'
  );
  const [skippedOnboarding, setSkippedOnboarding] = useState(
    () => localStorage.getItem('wtf:skippedOnboarding') === 'true'
  );

  useEffect(() => {
    function handleStorageReset(e: StorageEvent) {
      if (e.key === 'wtf:onboarded' && e.newValue === null) {
        setOnboarded(false);
        setSkippedOnboarding(false);
        setView('main');
      }
    }
    window.addEventListener('storage', handleStorageReset);
    return () => window.removeEventListener('storage', handleStorageReset);
  }, []);

  if (!onboarded) {
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

  if (view === 'settings') {
    return (
      <SettingsView
        onBack={() => setView('main')}
        onResetOnboarding={() => {
          localStorage.removeItem('wtf:onboarded');
          localStorage.removeItem('wtf:skippedOnboarding');
          setOnboarded(false);
          setSkippedOnboarding(false);
          setView('main');
        }}
      />
    );
  }

  return (
    <FramelessOverlay
      search={search}
      indexing={indexing}
      ollamaStatus={ollamaStatus}
      showSkipWarning={skippedOnboarding && settings.roots.length === 0}
      onOpenSettings={() => setView('settings')}
    />
  );
}

export default function App() {
  return (
    <IndexingProvider>
      <AppInner />
    </IndexingProvider>
  );
}
