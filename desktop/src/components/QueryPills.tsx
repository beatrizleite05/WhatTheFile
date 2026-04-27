import { motion, AnimatePresence } from 'framer-motion';
import { X } from 'lucide-react';
import { mediaTypePillColors, pillColorVar } from '../utils';
import type { ParsedQuery } from '../core/types';

interface QueryPillsProps {
  parsedRequest: ParsedQuery | null;
  onRemove: (field: keyof ParsedQuery, value: string) => void;
}

interface PillProps {
  label: string;
  bg: string;
  text: string;
  onRemove: () => void;
  ariaLabel: string;
}

function Pill({ label, bg, text, onRemove, ariaLabel }: PillProps) {
  return (
    <motion.span
      layout
      initial={{ opacity: 0, scale: 0.85 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.85 }}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: '2px 8px 2px 10px',
        borderRadius: 'var(--radius-pill)',
        background: bg,
        color: text,
        fontSize: 'var(--font-size-xs)',
        fontWeight: 500,
      }}
    >
      {label}
      <button
        aria-label={ariaLabel}
        onClick={onRemove}
        style={{
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          color: 'inherit',
          padding: 0,
          display: 'flex',
          alignItems: 'center',
          opacity: 0.7,
        }}
      >
        <X size={10} />
      </button>
    </motion.span>
  );
}

export function QueryPills({ parsedRequest, onRemove }: QueryPillsProps) {
  if (!parsedRequest) return null;

  const { mediaTypes, dateFrom, dateTo, rootScope, minConfidence } = parsedRequest;
  const hasPills = mediaTypes.length > 0 || dateFrom || dateTo || rootScope.length > 0 || minConfidence > 0;

  if (!hasPills) return null;

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, alignItems: 'center' }}>
      <AnimatePresence>
        {mediaTypes.map((mt) => {
          const colors = mediaTypePillColors(mt);
          return (
            <Pill
              key={`mt-${mt}`}
              label={mt}
              bg={colors.bg}
              text={colors.text}
              onRemove={() => onRemove('mediaTypes', mt)}
              ariaLabel={`Remove ${mt}`}
            />
          );
        })}

        {dateFrom && (
          <Pill
            key="dateFrom"
            label={`from ${dateFrom}`}
            bg={pillColorVar('date').bg}
            text={pillColorVar('date').text}
            onRemove={() => onRemove('dateFrom', dateFrom)}
            ariaLabel="Remove date filter"
          />
        )}

        {dateTo && (
          <Pill
            key="dateTo"
            label={`to ${dateTo}`}
            bg={pillColorVar('date').bg}
            text={pillColorVar('date').text}
            onRemove={() => onRemove('dateTo', dateTo)}
            ariaLabel="Remove date filter"
          />
        )}

        {rootScope.map((scope) => (
          <Pill
            key={`scope-${scope}`}
            label={scope}
            bg={pillColorVar('scope').bg}
            text={pillColorVar('scope').text}
            onRemove={() => onRemove('rootScope', scope)}
            ariaLabel={`Remove scope ${scope}`}
          />
        ))}

        {minConfidence > 0 && (
          <Pill
            key="confidence"
            label={`≥${Math.round(minConfidence * 100)}%`}
            bg={pillColorVar('confidence').bg}
            text={pillColorVar('confidence').text}
            onRemove={() => onRemove('minConfidence', String(minConfidence))}
            ariaLabel="Remove confidence filter"
          />
        )}


      </AnimatePresence>
    </div>
  );
}
