import { FiX } from "react-icons/fi";
import { Trans } from "@lingui/macro";
import { Link } from "react-router-dom";

import { HeaderLink } from "./HeaderLink";
import "./Header.scss";
import ExternalLink from "components/ExternalLink/ExternalLink";
import logoImg from "img/logo_GMSOL.png";

interface Props {
  small?: boolean;
  clickCloseIcon?: () => void;
  openSettings?: () => void;
}

export function AppHeaderLinks({ small, openSettings, clickCloseIcon }: Props) {
  return (
    <div className="App-header-links">
      {small && (
        <div className="App-header-links-header">
          <Link className="App-header-link-main" to="/">
            <img src={logoImg} height="21.462" alt="GMSOL Logo" />
          </Link>
          <div
            className="App-header-menu-icon-block mobile-cross-menu"
            onClick={() => clickCloseIcon && clickCloseIcon()}
          >
            <FiX className="App-header-menu-icon" />
          </div>
        </div>
      )}
      <div className="App-header-link-container">
        <HeaderLink to="/dashboard">
          <Trans>Dashboard</Trans>
        </HeaderLink>
      </div>
      <div className="App-header-link-container">
        <HeaderLink to="/earn">
          <Trans>Earn</Trans>
        </HeaderLink>
      </div>
      {/* <div className="App-header-link-container">
        <HeaderLink to="/buy">
          <Trans>Buy</Trans>
        </HeaderLink>
      </div> */}
      {/* <div className="App-header-link-container">
        <HeaderLink to="/referrals">
          <Trans>Referrals</Trans>
        </HeaderLink>
      </div>
      <div className="App-header-link-container">
        <HeaderLink to="/ecosystem">
          <Trans>Ecosystem</Trans>
        </HeaderLink>
      </div> */}
      <div className="App-header-link-container">
        <ExternalLink href="#">
          <Trans>Docs</Trans>
        </ExternalLink>
      </div>
      {small && (
        <div className="App-header-link-container">
          <a href="#" onClick={openSettings}>
            <Trans>Settings</Trans>
          </a>
        </div>
      )}
    </div>
  );
}
