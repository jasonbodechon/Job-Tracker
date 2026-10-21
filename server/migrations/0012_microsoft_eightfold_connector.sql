UPDATE company_directory
SET connector_class='Public site feed',
    careers_url='https://apply.careers.microsoft.com/careers?domain=microsoft.com',
    verification_status='Pending',
    verification_error=NULL,
    next_verification_at=now(),
    updated_at=now()
WHERE id='microsoft';
