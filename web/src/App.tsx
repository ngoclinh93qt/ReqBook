import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { IndexPage } from './pages/IndexPage';
import { SpecPage } from './pages/SpecPage';

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<IndexPage />} />
        <Route path="/spec/*" element={<SpecPage />} />
      </Routes>
    </BrowserRouter>
  );
}
